mod nfsv3 {
    use std::io::Cursor;

    use nfs_mamont::nfsv3::{createhow3, set_atime, set_mtime, NFS3_CREATEVERFSIZE};
    use nfs_mamont::nfsv3::{ftype3, mknoddata3};
    use nfs_mamont::parser::nfsv3::specdata3;
    use nfs_mamont::parser::nfsv3::{
        createhow3, devicedata3, diropargs3, mknoddata3, nfs_fh3, nfstime, sattr3, set_atime,
        set_mtime, symlinkdata3,
    };
    use nfs_mamont::parser::primitive::variant;
    use nfs_mamont::parser::Error;

    #[test]
    fn test_parse_specdata3_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = specdata3(&mut src).unwrap();
        assert_eq!(result.specdata1, 1);
        assert_eq!(result.specdata2, 2);
    }

    #[test]
    fn test_specdata3_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(specdata3(&mut src), Err(Error::IO(_))));
    }

    #[test]
    fn test_nfstime_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);

        let result = nfstime(&mut src).unwrap();
        assert_eq!(result.seconds, 1);
        assert_eq!(result.nseconds, 2);
    }

    #[test]
    fn test_nfstime_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(nfstime(&mut src), Err(Error::IO(_))));
    }

    #[test]
    fn test_set_atime_all_cases() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = set_atime(&mut src).unwrap();
        match result {
            set_atime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(set_atime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_set_mtime_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = set_mtime(&mut src).unwrap();
        match result {
            set_mtime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(set_mtime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_sattr3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        let mut src = Cursor::new(&data);

        let result = sattr3(&mut src).unwrap();
        assert!(result.mode.is_none());
        assert_eq!(result.uid, Some(1));
        assert!(result.gid.is_none());
        assert_eq!(result.size, Some(1));
        assert!(matches!(result.atime, set_atime::DONT_CHANGE));
        assert!(matches!(result.mtime, set_mtime::SET_TO_SERVER_TIME));
    }

    #[test]
    fn test_nfs_fh3_success() {
        let data = [0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = nfs_fh3(&mut src).unwrap();
        assert_eq!(result.data, [0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_nfs_fh3_badfh() {
        let data = [0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = nfs_fh3(&mut src);
        assert!(matches!(result, Err(Error::BadFileHandle)));
    }

    #[test]
    fn test_diropargs3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x08, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x03, b'a', b'b', b'c', 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = diropargs3(&mut src).unwrap();
        assert_eq!(result.dir.data, [0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        assert_eq!(result.name, "abc");
    }

    #[test]
    fn test_createhow3_unchecked() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = createhow3(&mut src).unwrap();
        assert!(matches!(result, createhow3::UNCHECKED(_)));

        let data = [
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        ];
        let mut src = Cursor::new(&data);

        let result = createhow3(&mut src).unwrap();
        match result {
            createhow3::EXCLUSIVE(verifier) => {
                assert_eq!(verifier.len(), NFS3_CREATEVERFSIZE);
                assert_eq!(verifier[0], 0x01);
            }
            _ => panic!("Expected EXCLUSIVE"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);

        assert!(matches!(createhow3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_symlinkdata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
            b'h', b'e', b'l', b'l', b'o', 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = symlinkdata3(&mut src).unwrap();
        assert_eq!(result.symlink_data, "hello");
    }

    #[test]
    fn test_devicedata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);

        let result = devicedata3(&mut src).unwrap();
        assert_eq!(result.spec.specdata1, 1);
        assert_eq!(result.spec.specdata2, 2);
    }

    #[test]
    fn test_mknoddata3_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3REG));

        let data = [0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3DIR));

        let data = [
            0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);
        let result = mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3BLK(_)));

        let data = [0x00, 0x00, 0x00, 0x05];
        let mut src = Cursor::new(&data);
        let result = mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3LNK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        assert!(matches!(mknoddata3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_c_enum() {
        let data = [0x00, 0x00, 0x00, 0x06];
        let mut src = Cursor::new(&data);
        let result = variant(&mut src).unwrap();
        assert!(matches!(result, ftype3::NF3SOCK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        let result = variant::<ftype3>(&mut src);
        assert!(matches!(result, Err(Error::EnumDiscMismatch)));
    }
}

mod primitive {
    use std::io::Cursor;

    use byteorder::{BigEndian, WriteBytesExt};

    use nfs_mamont::parser::primitive::{
        array, bool, option, string, string_max_size, u32, u64, u8, vector,
    };
    use nfs_mamont::parser::Error;

    #[test]
    fn test_u32() {
        let init = [0u32, 7, 788965];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(i).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = u32(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_u64() {
        let init = [2u64, 0, 125, 78569];
        let mut src = Vec::with_capacity(size_of::<u64>() * init.len());
        for i in init {
            src.write_u64::<BigEndian>(i).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = u64(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_bool() {
        let init = [true, false, true];
        let mut src = Vec::with_capacity(size_of::<u32>() * init.len());
        for i in init {
            src.write_u32::<BigEndian>(if i { 1 } else { 0 }).unwrap();
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = bool(&mut src).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_option() {
        let init = [None, Some(85u32), Some(0)];
        let mut src = Vec::new();
        for op in init {
            if let Some(val) = op {
                src.write_u32::<BigEndian>(1).unwrap();
                src.write_u32::<BigEndian>(val).unwrap();
            } else {
                src.write_u32::<BigEndian>(0).unwrap();
            }
        }
        let mut src = Cursor::new(src);
        for correct_res in init {
            let val = option(&mut src, |s| u32(s)).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_array_u32() {
        let init = [457u32, 475, 0];
        let mut src = Vec::new();
        let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let mut src = Cursor::new(src);
        let val = array::<3, u32>(&mut src, |s| u32(s)).expect("Cannot parse value!");
        assert_eq!(val, init)
    }

    #[test]
    fn test_vec_u8() {
        let init = vec![1u8, 2, 3, 4, 5];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let padding_len = (4 - (init.len() % 4)) % 4;
        src.extend(vec![0u8; padding_len]);

        let result = vector(&mut Cursor::new(src), |s| u8(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_vec_u8_with_padding() {
        let init = vec![1u8, 2, 3];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        src.push(0);
        let result = vector(&mut Cursor::new(src), |s| u8(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_u8_array_padding_error() {
        let init = [1u8, 2, 3];
        let mut src = Vec::new();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let result = array::<3, u8>(&mut Cursor::new(src), |s| u8(s));
        assert!(matches!(result, Err(Error::IncorrectPadding)));
    }

    #[test]
    fn test_u8_array_miss_elements() {
        let init = [78u32, 0, 78965];
        let mut src = Vec::new();
        let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let result = array::<4, u32>(&mut Cursor::new(src), |s| u32(s));
        assert!(matches!(result, Err(Error::IO(_))));
    }

    #[test]
    fn test_vec_u32() {
        let init = vec![457u32, 475, 0, 42];
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(init.len() as u32).unwrap();
        for i in &init {
            src.write_u32::<BigEndian>(*i).unwrap();
        }
        let result = vector(&mut Cursor::new(src), |s| u32(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_string_utf8_error() {
        let mut src = Vec::new();
        let invalid_utf8 = vec![0xFF, 0xFF, 0xFF];
        src.write_u32::<BigEndian>(invalid_utf8.len() as u32).unwrap();
        src.extend_from_slice(&invalid_utf8);
        src.push(0);
        let result = string(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IncorrectString(_))));
    }

    #[test]
    fn test_string_valid() {
        let test_string = "test string".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        src.write_u8(0u8).unwrap();
        let result = string(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, test_string);
    }

    #[test]
    fn test_string_with_max_len_valid() {
        let test_string = "test".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let result = string_max_size(&mut Cursor::new(src), 10).unwrap();
        assert_eq!(result, test_string);
    }

    #[test]
    fn test_string_with_max_len_too_long() {
        let test_string = "this string is too long".to_string();
        let mut src = Vec::new();

        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let padding_len = (4 - (test_string.len() % 4)) % 4;
        src.extend(vec![0u8; padding_len]);

        let result = string_max_size(&mut Cursor::new(src), 10);
        assert!(matches!(result, Err(Error::MaxELemLimit)));
    }

    #[test]
    fn test_read_error() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(10).unwrap();
        let result = vector(&mut Cursor::new(src), |s| u8(s));
        assert!(matches!(result, Err(Error::IO(_))));
    }
}
