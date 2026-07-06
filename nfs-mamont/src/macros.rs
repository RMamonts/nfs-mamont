/// Conditional pub use macro that exports items only when a specific feature is enabled.
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
