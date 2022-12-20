// Some lints only available in nightly, want to keep the rule enabled so they go into action when stable
#![allow(unknown_lints)]
#![warn(missing_docs)]
#![warn(rustdoc::all)]
#![warn(absolute_paths_not_starting_with_crate)]
#![warn(elided_lifetimes_in_paths)]
#![warn(explicit_outlives_requirements)]
#![warn(fuzzy_provenance_casts)]
#![warn(let_underscore_drop)]
#![warn(lossy_provenance_casts)]
#![warn(macro_use_extern_crate)]
#![warn(meta_variable_misuse)]
#![warn(missing_debug_implementations)]
#![warn(missing_copy_implementations)]
#![warn(must_not_suspend)]
#![warn(non_ascii_idents)]
#![warn(non_exhaustive_omitted_patterns)]
#![warn(noop_method_call)]
#![warn(pointer_structural_match)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_crate_dependencies)]
#![warn(unused_extern_crates)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_macro_rules)]
#![warn(unused_qualifications)]
#![warn(unused_tuple_struct_fields)]
#![warn(variant_size_differences)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(clippy::as_underscore)]
#![warn(clippy::assertions_on_result_states)]
#![warn(clippy::dbg_macro)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::default_numeric_fallback)]
#![warn(clippy::default_union_representation)]
#![warn(clippy::deref_by_slicing)]
#![warn(clippy::disallowed_script_idents)]
#![warn(clippy::else_if_without_else)]
#![warn(clippy::empty_drop)]
#![warn(clippy::empty_structs_with_brackets)]
#![warn(clippy::exit)]
#![warn(clippy::float_cmp_const)]
#![warn(clippy::format_push_string)]
#![warn(clippy::get_unwrap)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::integer_division)]
#![warn(clippy::let_underscore_must_use)]
#![warn(clippy::lossy_float_literal)]
#![warn(clippy::map_err_ignore)]
#![warn(clippy::mem_forget)]
#![warn(clippy::mixed_read_write_in_expression)]
#![warn(clippy::mod_module_files)]
#![warn(clippy::multiple_inherent_impl)]
#![warn(clippy::non_ascii_literal)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::partial_pub_fields)]
#![warn(clippy::pattern_type_mismatch)]
#![warn(clippy::print_stdout)]
#![warn(clippy::print_stderr)]
#![warn(clippy::rc_buffer)]
#![warn(clippy::rc_mutex)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::same_name_method)]
#![warn(clippy::unseparated_literal_suffix)]
#![warn(clippy::single_char_lifetime_names)]
#![warn(clippy::str_to_string)]
#![warn(clippy::string_add)]
#![warn(clippy::string_slice)]
#![warn(clippy::string_to_string)]
#![warn(clippy::suspicious_xor_used_as_pow)]
#![warn(clippy::todo)]
#![warn(clippy::try_err)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![warn(clippy::unimplemented)]
#![warn(clippy::unnecessary_self_imports)]
#![warn(clippy::unneeded_field_pattern)]
#![warn(clippy::unwrap_in_result)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::use_debug)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::clone_on_ref_ptr)]
// Usually intentional
#![allow(clippy::future_not_send)]
// Cleaner in some cases
#![allow(clippy::match_bool)]
// Better with repetition
#![allow(clippy::module_name_repetitions)]
// I like my complicated functions
#![allow(clippy::too_many_lines)]
// It useful
#![allow(clippy::wildcard_imports)]
// Prefered over not having pub(crate) and being unclear about visibility
#![allow(clippy::redundant_pub_crate)]
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[cfg(target_arch = "wasm32")]
mod browser;

mod atomic_id;
mod futures;
mod listenable;
mod ptr_hash;
mod sleep;
mod weak_link;

/// Cache Queries and Mutations
pub mod cache;
/// [`crate::client::QueryClient`]
pub mod client;
/// Configuration
pub mod config;
/// Const default trait
pub mod const_default;
/// [`crate::mutation::Mutation`]
pub mod mutation;
/// [`crate::query::Query`]
pub mod query;
/// Query statuses
pub mod status;

/// Hydration API
#[cfg(feature = "hydrate")]
pub mod hydrate;

/// Sycamore API
#[cfg(feature = "sycamore")]
pub mod sycamore;

#[cfg(test)]
mod tests {
    use crate::{client::QueryClient, query::Query, status::FetchResult};

    fn check<E>(res: FetchResult<i32, E>, exp: i32) {
        match res {
            FetchResult::Fresh(Ok(f)) => assert_eq!(exp, *f),
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn multiple_client_query() {
        let client1 = QueryClient::default();
        let client2 = QueryClient::default();
        let query1 = Query::new(|| Box::pin(async { Ok::<i32, ()>(12345_i32) }));
        let query2 = Query::new(|| Box::pin(async { Ok::<i32, ()>(67890_i32) }));

        check(client1.fetch(&query1).await, 12345_i32);
        check(client2.fetch(&query1).await, 12345_i32);
        check(client1.fetch(&query2).await, 67890_i32);
        check(client2.fetch(&query2).await, 67890_i32);
    }
}
