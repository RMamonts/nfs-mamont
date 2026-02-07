use std::io::{Result, Write};

use crate::serializer::nfs::file_handle;
use crate::serializer::nfs::files::file_name;
use crate::vfs::rename::Args;

pub fn rename_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    file_handle(dest, arg.from_dir)
        .and_then(|_| file_name(dest, arg.from_name))
        .and_then(|_| file_handle(dest, arg.to_dir))
        .and_then(|_| file_name(dest, arg.to_name))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::client::arguments::rename::rename_args;
    use crate::vfs::file::{FileName, Handle};
    use crate::vfs::rename;

    #[test]
    fn rename_arg_serialization() {
        let arg = rename::Args {
            from_dir: Handle([0, 0, 0, 0, 0, 0, 0, 0]),
            from_name: FileName(" ".to_string()),
            to_dir: Handle([0, 0, 0, 0, 0, 0, 0, 0]),
            to_name: FileName(" ".to_string()),
        };
        let mut cursor = Cursor::new(vec![0u8; 40]);
        rename_args(&mut cursor, arg).unwrap();
        assert_eq!(
            cursor.into_inner(),
            vec![
                0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, b' ', 0, 0, 0, 0, 0, 0, 8, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0, 0, 1, b' ', 0, 0, 0
            ]
        )
    }
}
