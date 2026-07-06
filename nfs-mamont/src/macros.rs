/// Conditional pub use macro that exports items only when a specific feature is enabled.
///
/// # Examples
/// ```ignore
/// // Exports only when 'arbitrary' feature is enabled
/// pub_use_if_feature!("arbitrary", some_module::SomeType);
///
/// // Multiple items including re-exports with 'as'
/// pub_use_if_feature!(
///     "arbitrary",
///     item1,
///     item2,
///     module::item3 as alias,
///     module::submodule as sub_alias
/// );
/// ```
#[macro_export]
macro_rules! pub_use_if_feature {
    // Single feature, single or multiple items (with optional 'as' aliases)
    ($feature:literal, $($item:path $(as $alias:ident)?),+ $(,)?) => {
        $(
            #[cfg(feature = $feature)]
            pub use $item $( as $alias)?;
        )+
    };
}

/// Conditional pub use for re-exporting multiple items from a module.
///
/// # Examples
/// ```ignore
/// // Re-export all public items from a module only when feature is enabled
/// pub_use_module_if_feature!("test", module as mod_name);
/// ```
#[macro_export]
macro_rules! pub_use_module_if_feature {
    ($feature:literal, $module:path as $alias:ident) => {
        #[cfg(feature = $feature)]
        pub use $module as $alias;
    };
    ($feature:literal, $module:path) => {
        #[cfg(feature = $feature)]
        pub use $module;
    };
}
