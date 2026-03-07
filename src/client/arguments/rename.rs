use std::io::{Result, Write};

use crate::serializer::files::dir_op_arg;
use crate::vfs::rename::Args;

pub fn rename_args(dest: &mut impl Write, arg: Args) -> Result<()> {
    dir_op_arg(dest, arg.from).and_then(|_| dir_op_arg(dest, arg.to))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::client::arguments::rename::rename_args;
    use crate::vfs::file::Handle;
    use crate::vfs::{file, rename, DirOpArgs};

    #[test]
    fn rename_arg_serialization() {
        let arg = rename::Args {
            from: DirOpArgs {
                dir: Handle([0, 0, 0, 0, 0, 0, 0, 0]),
                name: file::Name::new(" ".to_string()).unwrap(),
            },
            to: DirOpArgs {
                dir: Handle([0, 0, 0, 0, 0, 0, 0, 0]),
                name: file::Name::new(" ".to_string()).unwrap(),
            },
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
