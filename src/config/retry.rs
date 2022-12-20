use std::{
    fmt::{self, Debug, Formatter},
    rc::Rc,
    time::Duration,
};

use crate::const_default::ConstDefault;

type DelayFn<'func, E> = Rc<dyn Fn(u32, Rc<E>) -> Duration + 'func>;
type RetryFn<'func, E> = Rc<dyn Fn(u32, Rc<E>) -> bool + 'func>;

// Already small as possible
#[allow(variant_size_differences)]
/// Control how to retry a query or mutation
/// Default: retry 3 times
pub enum RetryPolicy<'func, E: ?Sized> {
    /// Retry when the closure returns true, given the failure count and error
    Func(RetryFn<'func, E>),
    /// Retry infinitely
    Infinite,
    /// Retry for a set number of times
    Num(u32),
}

impl<E: ?Sized> Clone for RetryPolicy<'_, E> {
    fn clone(&self) -> Self {
        match *self {
            Self::Func(ref func) => Self::Func(Rc::clone(func)),
            Self::Infinite => Self::Infinite,
            Self::Num(n) => Self::Num(n),
        }
    }
}

impl<E: ?Sized> Debug for RetryPolicy<'_, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Func(_) => f.debug_tuple("RetryPolicy::Func").field(&"..").finish(),
            Self::Infinite => f.debug_tuple("RetryPolicy::Infinite").finish(),
            Self::Num(ref n) => f.debug_tuple("RetryPolicy::Num").field(n).finish(),
        }
    }
}

impl<E: ?Sized> Default for RetryPolicy<'_, E> {
    fn default() -> Self {
        Self::const_default()
    }
}

impl<E: ?Sized> ConstDefault for RetryPolicy<'_, E> {
    const DEFAULT: Self = Self::const_default();
}

impl<E: ?Sized> RetryPolicy<'_, E> {
    /// Gets default for [`RetryPolicy`] as a const
    #[must_use = "Gets the default, has no effect if unused"]
    #[inline]
    pub const fn const_default() -> Self {
        Self::Num(3)
    }
}

/// Control how long between retries
/// Default: Backoff, starting at 1000ms with a maximum of 30s
pub enum RetryDelay<'func, E: ?Sized> {
    /// Double time between retires
    Backoff {
        /// First amount of time to wait before retrying, will be doubled for each failure
        initial: Duration,
        /// Don't go above this amount of time
        maximum: Duration,
    },
    /// Always wait a set time between retries
    Always(Duration),
    /// Retry after the time returned from the closure, given the failure count and error
    DelayFn(DelayFn<'func, E>),
}

impl<E: ?Sized> Clone for RetryDelay<'_, E> {
    fn clone(&self) -> Self {
        match *self {
            Self::DelayFn(ref func) => Self::DelayFn(Rc::clone(func)),
            Self::Backoff { initial, maximum } => Self::Backoff { initial, maximum },
            Self::Always(a) => Self::Always(a),
        }
    }
}

impl<E: ?Sized> Debug for RetryDelay<'_, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Backoff {
                ref initial,
                ref maximum,
            } => f
                .debug_struct("RetryDelay::Backoff")
                .field("initial", initial)
                .field("maximum", maximum)
                .finish(),
            Self::Always(ref dur) => f.debug_tuple("RetryDelay::Always").field(dur).finish(),
            Self::DelayFn(_) => f.debug_tuple("RetryDelay::DelayFn").field(&"..").finish(),
        }
    }
}

impl<E: ?Sized> Default for RetryDelay<'_, E> {
    fn default() -> Self {
        Self::const_default()
    }
}

impl<E: ?Sized> ConstDefault for RetryDelay<'_, E> {
    const DEFAULT: Self = Self::const_default();
}

impl<E: ?Sized> RetryDelay<'_, E> {
    /// Gets default for [`RetryDelay`] as a const
    #[must_use = "Gets the default, has no effect if unused"]
    #[inline]
    pub const fn const_default() -> Self {
        Self::Backoff {
            initial: Duration::from_millis(1000),
            maximum: Duration::from_secs(30),
        }
    }
}

/// Configuration for how queries and mutations are retired
#[derive(Debug)]
pub struct RetryConfig<'func, E: ?Sized> {
    /// See [`RetryPolicy`]
    pub policy: RetryPolicy<'func, E>,
    /// See [`RetryDelay`]
    pub delay: RetryDelay<'func, E>,
}

impl<E: ?Sized> Default for RetryConfig<'_, E> {
    fn default() -> Self {
        Self {
            policy: RetryPolicy::default(),
            delay: RetryDelay::default(),
        }
    }
}

impl<E: ?Sized> Clone for RetryConfig<'_, E> {
    fn clone(&self) -> Self {
        Self {
            policy: self.policy.clone(),
            delay: self.delay.clone(),
        }
    }
}

impl<E: ?Sized> ConstDefault for RetryConfig<'_, E> {
    const DEFAULT: Self = Self::const_default();
}

impl<'func, E: ?Sized> RetryConfig<'func, E> {
    /// Gets default for [`RetryConfig`] as a const
    #[must_use = "Gets the default, has no effect if unused"]
    #[inline]
    pub const fn const_default() -> Self {
        Self {
            policy: RetryPolicy::const_default(),
            delay: RetryDelay::const_default(),
        }
    }

    /// Creates a retry policy that doesn't retry
    /// Delay is set to default
    #[must_use = "No reason to create if not used"]
    pub fn none() -> Self {
        Self {
            policy: RetryPolicy::Num(0),
            delay: RetryDelay::default(),
        }
    }

    /// Set the retry policy to infinite
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn infinite(mut self) -> Self {
        self.policy = RetryPolicy::Infinite;
        self
    }

    /// Set the retry policy to `num` times
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn num(mut self, num: u32) -> Self {
        self.policy = RetryPolicy::Num(num);
        self
    }

    /// Set the retry policy to use the provided closure
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn policy_fn(mut self, func: impl Fn(u32, Rc<E>) -> bool + 'func) -> Self {
        self.policy = RetryPolicy::Func(Rc::new(func));
        self
    }

    /// Set the retry delay to backoff with the provided parameters
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn backoff(mut self, initial: Duration, maximum: Duration) -> Self {
        self.delay = RetryDelay::Backoff { initial, maximum };
        self
    }

    /// Set the retry delay to always be `duration`
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn always(mut self, duration: Duration) -> Self {
        self.delay = RetryDelay::Always(duration);
        self
    }

    /// Set the retry delay to use the provided closure
    // Possible drop, can't be const
    #[allow(clippy::missing_const_for_fn)]
    #[must_use = "Builder pattern"]
    #[inline]
    pub fn delay_fn(mut self, func: impl Fn(u32, Rc<E>) -> Duration + 'func) -> Self {
        self.delay = RetryDelay::DelayFn(Rc::new(func));
        self
    }
}

impl<E: ?Sized> RetryConfig<'_, E> {
    pub(crate) fn retry_delay(&self, failure_count: u32, error: Rc<E>) -> Option<Duration> {
        match self.policy {
            RetryPolicy::Func(ref func) if func(failure_count, Rc::clone(&error)) => Some(()),
            RetryPolicy::Infinite => Some(()),
            RetryPolicy::Num(ref n) if failure_count <= *n => Some(()),
            _ => None,
        }?;

        Some(match self.delay {
            RetryDelay::Always(ref d) => *d,
            RetryDelay::Backoff {
                ref initial,
                ref maximum,
            } => initial
                .saturating_mul(2_u32.pow(failure_count.saturating_sub(1)))
                .min(*maximum),
            RetryDelay::DelayFn(ref func) => func(failure_count, error),
        })
    }
}
