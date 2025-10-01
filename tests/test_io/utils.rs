use std::path::Path;
use std::path::PathBuf;

pub fn join(components: &[&dyn AsRef<Path>]) -> PathBuf {
    let mut path = PathBuf::new();

    for component in components {
        path.push(component.as_ref());
    }

    path
}
