use std::{fmt::Debug, time::Duration};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::const_default::ConstDefault;

/// Query/Mutation return error
pub mod error;
/// Handle retries
pub mod retry;

pub(crate) mod resolve;

/// A configuration option with the ability to be default ([`Self::default()`] or [`Self::const_default()`]), inherrit, or set
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum SetOption<T> {
    /// Will inherrit configuration option from lower priority configuration
    Inherrit,
    /// Will use this option
    Set(T),
}

impl<T: Default> Default for SetOption<T> {
    fn default() -> Self {
        Self::set(T::default())
    }
}

impl<T: ConstDefault> ConstDefault for SetOption<T> {
    const DEFAULT: Self = Self::const_default();
}

impl<T: ConstDefault> SetOption<T> {
    /// Gets default for [`T`] as a const [`Self::Set(T)`]
    pub const fn const_default() -> Self {
        Self::set(T::DEFAULT)
    }
}

impl<T> SetOption<T> {
    /// Creates new option that will inherrit
    #[inline]
    #[must_use = "No need to create if you don't use it"]
    pub const fn inherrit() -> Self {
        Self::Inherrit
    }

    /// Creates new option that will use `value`
    #[inline]
    #[must_use = "No need to create if you don't use it"]
    pub const fn set(value: T) -> Self {
        Self::Set(value)
    }
}

/// Configuration for the length of time inactive queries/mutations remain cached
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum CacheTime {
    /// Permanently remains in cache
    Infinite,
    /// Remains in cache for `Duration`
    Duration(Duration),
}

impl Default for CacheTime {
    fn default() -> Self {
        Self::const_default()
    }
}

impl ConstDefault for CacheTime {
    const DEFAULT: Self = Self::const_default();
}

impl CacheTime {
    /// Gets default for [`CacheTime`] as a const
    #[must_use = "Gets the default, has no effect if unused"]
    #[inline]
    pub const fn const_default() -> Self {
        Self::Duration(Duration::from_secs(5 * 60))
    }
}

/// Setting for how [`QueryClient`] should handle being offline in the browser
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum NetworkMode {
    /// Only execute query when there is an internet connection, otherwise fetch status set to paused until connection returns, at which point the request is made
    Online,
    /// Ignore online status
    Always,
    /// If there is no connection, try once and pause if it fails
    OfflineFirst,
}

impl ConstDefault for NetworkMode {
    const DEFAULT: Self = Self::const_default();
}

impl Default for NetworkMode {
    fn default() -> Self {
        Self::const_default()
    }
}

impl NetworkMode {
    pub const fn const_default() -> Self {
        Self::Online
    }

    pub(crate) const fn should_try(self, count: u32) -> bool {
        match self {
            Self::Always => true,
            Self::OfflineFirst if count == 0 => true,
            _ => false,
        }
    }
}
