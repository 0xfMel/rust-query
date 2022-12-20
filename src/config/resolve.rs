use std::{rc::Rc, time::Duration};

use downcast_rs::{impl_downcast, Downcast};

use crate::{client::ClientOpts, config::SetOption, mutation::MutationOpts, query::QueryOpts};

use super::{error::Error, retry::RetryConfig, CacheTime, NetworkMode};

pub trait ConfigOpt: Downcast {}
impl_downcast!(ConfigOpt);

#[derive(Clone, Copy)]
pub(crate) enum ConfigOption {
    CacheTime,
    NetworkMode,
}

#[inline]
pub(crate) fn resolve_option<T: ConfigOpt + Default + Clone>(
    opt: ConfigOption,
    client: &ClientOpts<'_>,
    action: &impl ClientAction,
) -> T {
    resolve_option_inner(opt, client, action).map_or_else(T::default, |option| {
        let val: &T = option
            .downcast_ref()
            .expect("T should correspond to the correct type for `opt`");
        val.clone()
    })
}

fn resolve_option_inner<'client, 'query, 'res>(
    opt: ConfigOption,
    client: &'client ClientOpts<'_>,
    action: &'query impl ClientAction,
) -> Option<&'res dyn ConfigOpt>
where
    'client: 'res,
    'query: 'res,
{
    if let Some(option) = action.get(opt) {
        return Some(option);
    }

    match action.action_type() {
        ActionType::Query => {
            if let Some(ref client_query) = client.query {
                if let Some(option) = client_query.get(opt) {
                    return Some(option);
                }
            }
        }
        ActionType::Mutation => {
            if let Some(ref client_mutation) = client.mutation {
                if let Some(option) = client_mutation.get(opt) {
                    return Some(option);
                }
            }
        }
    }

    if let Some(option) = client.get(opt) {
        return Some(option);
    }

    None
}

pub(crate) enum RetryType<'cfg, 'func, E> {
    Concrete(RetryConfig<'cfg, E>),
    TraitObject(RetryConfig<'cfg, dyn Error + 'func>),
}

impl<'func, E: Error + 'func> RetryType<'_, 'func, E> {
    pub(crate) fn retry_delay(&self, failure_count: u32, error: Rc<E>) -> Option<Duration> {
        match *self {
            Self::Concrete(ref c) => c.retry_delay(failure_count, error),
            Self::TraitObject(ref t) => t.retry_delay(failure_count, error),
        }
    }
}

pub(crate) fn resolve_retry<'client, 'query, 'res, 'func, E>(
    client: &'client ClientOpts<'func>,
    query: &'query QueryOpts<'_, E>,
) -> RetryType<'res, 'func, E>
where
    'client: 'res,
    'query: 'res,
{
    if let SetOption::Set(ref retry) = query.retry {
        log::info!("using query");
        return RetryType::Concrete(retry.clone());
    }

    if let Some(ref client_query) = client.query {
        if let SetOption::Set(ref retry) = client_query.retry {
            log::info!("using client.query");
            return RetryType::TraitObject(retry.clone());
        }
    }

    if let SetOption::Set(ref retry) = client.retry {
        log::info!("using client");
        return RetryType::TraitObject(retry.clone());
    }

    log::info!("using default");
    RetryType::Concrete(RetryConfig::default())
}

impl ConfigOpt for CacheTime {}
impl ConfigOpt for NetworkMode {}

impl<T: ConfigOpt> SetOption<T> {
    fn as_option(&self) -> Option<&(dyn ConfigOpt)> {
        match *self {
            Self::Inherrit => None,
            Self::Set(ref s) => Some(s),
        }
    }
}

pub(crate) trait GetOption {
    fn get(&self, opt: ConfigOption) -> Option<&(dyn ConfigOpt)>;
}

pub(crate) enum ActionType {
    Query,
    Mutation,
}

pub(crate) trait ClientAction: GetOption {
    fn action_type(&self) -> ActionType;
}

impl<E: ?Sized> ClientAction for QueryOpts<'_, E> {
    fn action_type(&self) -> ActionType {
        ActionType::Query
    }
}

impl<E: ?Sized> ClientAction for MutationOpts<'_, E> {
    fn action_type(&self) -> ActionType {
        ActionType::Mutation
    }
}

impl GetOption for ClientOpts<'_> {
    fn get(&self, opt: ConfigOption) -> Option<&(dyn ConfigOpt)> {
        match opt {
            ConfigOption::CacheTime => self.cache_time.as_option(),
            ConfigOption::NetworkMode => self.network_mode.as_option(),
        }
    }
}

impl<E: ?Sized> GetOption for QueryOpts<'_, E> {
    fn get(&self, opt: ConfigOption) -> Option<&(dyn ConfigOpt)> {
        match opt {
            ConfigOption::CacheTime => self.cache_time.as_option(),
            ConfigOption::NetworkMode => self.network_mode.as_option(),
        }
    }
}

impl<E: ?Sized> GetOption for MutationOpts<'_, E> {
    fn get(&self, opt: ConfigOption) -> Option<&(dyn ConfigOpt)> {
        match opt {
            ConfigOption::CacheTime => self.cache_time.as_option(),
            ConfigOption::NetworkMode => self.network_mode.as_option(),
        }
    }
}
