mod nfsv3 {
    use nfs_mamont::nfsv3::{createhow3, set_atime, set_mtime, NFS3_CREATEVERFSIZE};
    use nfs_mamont::nfsv3::{ftype3, mknoddata3};
    use nfs_mamont::parser::nfsv3::parse_specdata3;
    use nfs_mamont::parser::nfsv3::{
        parse_createhow3, parse_devicedata3, parse_diropargs3, parse_mknoddata3, parse_nfs_fh3,
        parse_nfstime, parse_sattr3, parse_set_atime, parse_set_mtime, parse_symlinkdata3,
        MAX_FILEHANDLE,
    };
    use nfs_mamont::parser::primitive::parse_c_enum;
    use nfs_mamont::parser::Error;
    use std::io::Cursor;

    #[test]
    fn test_parse_specdata3_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_specdata3(&mut src).unwrap();
        assert_eq!(result.specdata1, 1);
        assert_eq!(result.specdata2, 2);
    }

    #[test]
    fn test_parse_specdata3_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_specdata3(&mut src), Err(Error::IO(_))));
    }

    #[test]
    fn test_parse_nfstime_success() {
        let data = [0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);

        let result = parse_nfstime(&mut src).unwrap();
        assert_eq!(result.seconds, 1);
        assert_eq!(result.nseconds, 2);
    }

    #[test]
    fn test_parse_nfstime_error() {
        let data = [0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_nfstime(&mut src), Err(Error::IO(_))));
    }

    #[test]
    fn test_parse_set_atime_all_cases() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        assert!(matches!(result, set_atime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_set_atime(&mut src).unwrap();
        match result {
            set_atime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_set_atime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_set_mtime_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x00];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::DONT_CHANGE));

        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        assert!(matches!(result, set_mtime::SET_TO_SERVER_TIME));

        let data = [0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_set_mtime(&mut src).unwrap();
        match result {
            set_mtime::SET_TO_CLIENT_TIME(nfstime) => {
                assert_eq!(nfstime.seconds, 1);
                assert_eq!(nfstime.nseconds, 2);
            }
            _ => panic!("Expected SET_TO_CLIENT_TIME"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_set_mtime(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_sattr3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x0, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_sattr3(&mut src).unwrap();
        assert!(result.mode.is_none());
        assert_eq!(result.uid, Some(1));
        assert!(result.gid.is_none());
        assert_eq!(result.size, Some(1));
        assert!(matches!(result.atime, set_atime::DONT_CHANGE));
        assert!(matches!(result.mtime, set_mtime::SET_TO_SERVER_TIME));
    }

    #[test]
    fn test_parse_nfs_fh3_success() {
        let data = [0x00, 0x00, 0x00, 0x03, 0x01, 0x02, 0x03, 0x00];
        let mut src = Cursor::new(&data);

        let result = parse_nfs_fh3(&mut src).unwrap();
        assert_eq!(result.data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_parse_nfs_fh3_exceeds_max_size() {
        let mut data = vec![0xFF, 0xFF, 0xFF, 0xFF];
        data.extend(vec![0x00; MAX_FILEHANDLE + 1]);

        let mut src = Cursor::new(&data);
        assert!(matches!(parse_nfs_fh3(&mut src), Err(Error::MaxELemLimit)));
    }

    #[test]
    fn test_parse_diropargs3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, b'a', b'b',
            b'c', 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_diropargs3(&mut src).unwrap();
        assert_eq!(result.dir.data, vec![0x01, 0x02]);
        assert_eq!(result.name, "abc");
    }

    #[test]
    fn test_parse_createhow3_unchecked() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_createhow3(&mut src).unwrap();
        assert!(matches!(result, createhow3::UNCHECKED(_)));

        let data = [
            0x00, 0x00, 0x00, 0x02, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
            0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_createhow3(&mut src).unwrap();
        match result {
            createhow3::EXCLUSIVE(verifier) => {
                assert_eq!(verifier.len(), NFS3_CREATEVERFSIZE);
                assert_eq!(verifier[0], 0x01);
            }
            _ => panic!("Expected EXCLUSIVE"),
        }

        let data = [0x00, 0x00, 0x00, 0x03];
        let mut src = Cursor::new(&data);

        assert!(matches!(parse_createhow3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_parse_symlinkdata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05,
            b'h', b'e', b'l', b'l', b'o', 0x00, 0x00, 0x00,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_symlinkdata3(&mut src).unwrap();
        assert_eq!(result.symlink_data, "hello");
    }

    #[test]
    fn test_parse_devicedata3_success() {
        let data = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);

        let result = parse_devicedata3(&mut src).unwrap();
        assert_eq!(result.spec.specdata1, 1);
        assert_eq!(result.spec.specdata2, 2);
    }

    #[test]
    fn test_parse_mknoddata3_all_variants() {
        let data = [0x00, 0x00, 0x00, 0x01];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3REG));

        let data = [0x00, 0x00, 0x00, 0x02];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3DIR));

        let data = [
            0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
        ];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3BLK(_)));

        let data = [0x00, 0x00, 0x00, 0x05];
        let mut src = Cursor::new(&data);
        let result = parse_mknoddata3(&mut src).unwrap();
        assert!(matches!(result, mknoddata3::NF3LNK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        assert!(matches!(parse_mknoddata3(&mut src), Err(Error::EnumDiscMismatch)));
    }

    #[test]
    fn test_c_enum() {
        let data = [0x00, 0x00, 0x00, 0x06];
        let mut src = Cursor::new(&data);
        let result = parse_c_enum(&mut src).unwrap();
        assert!(matches!(result, ftype3::NF3SOCK));

        let data = [0x00, 0x00, 0x00, 0x08];
        let mut src = Cursor::new(&data);
        let result = parse_c_enum::<ftype3>(&mut src);
        assert!(matches!(result, Err(Error::EnumDiscMismatch)));
    }
}

mod primitive {
    use std::io::Cursor;

    use byteorder::{BigEndian, WriteBytesExt};

    use nfs_mamont::parser::primitive::{
        parse_array, parse_bool, parse_option, parse_string, parse_string_max_len, parse_u32,
        parse_u64, parse_u8, parse_vector,
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
            let val = parse_u32(&mut src).expect("Cannot parse value!");
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
            let val = parse_u64(&mut src).expect("Cannot parse value!");
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
            let val = parse_bool(&mut src).expect("Cannot parse value!");
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
            let val = parse_option(&mut src, |s| parse_u32(s)).expect("Cannot parse value!");
            assert_eq!(val, correct_res)
        }
    }

    #[test]
    fn test_array_u32() {
        let init = [457u32, 475, 0];
        let mut src = Vec::new();
        init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let mut src = Cursor::new(src);
        let val = parse_array::<3, u32>(&mut src, |s| parse_u32(s)).expect("Cannot parse value!");
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

        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s)).unwrap();
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
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_u8_array_padding_error() {
        let init = [1u8, 2, 3];
        let mut src = Vec::new();
        for i in &init {
            src.write_u8(*i).unwrap();
        }
        let result = parse_array::<3, u8>(&mut Cursor::new(src), |s| parse_u8(s));
        assert!(matches!(result, Err(Error::IncorrectPadding)));
    }

    #[test]
    fn test_u8_array_miss_elements() {
        let init = [78u32, 0, 78965];
        let mut src = Vec::new();
        let _ = init.map(|i| src.write_u32::<BigEndian>(i).unwrap());
        let result = parse_array::<4, u32>(&mut Cursor::new(src), |s| parse_u32(s));
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
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u32(s)).unwrap();
        assert_eq!(result, init);
    }

    #[test]
    fn test_string_utf8_error() {
        let mut src = Vec::new();
        let invalid_utf8 = vec![0xFF, 0xFF, 0xFF];
        src.write_u32::<BigEndian>(invalid_utf8.len() as u32).unwrap();
        src.extend_from_slice(&invalid_utf8);
        src.push(0);
        let result = parse_string(&mut Cursor::new(src));
        assert!(matches!(result, Err(Error::IncorrectString(_))));
    }

    #[test]
    fn test_string_valid() {
        let test_string = "test string".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        src.write_u8(0u8).unwrap();
        let result = parse_string(&mut Cursor::new(src)).unwrap();
        assert_eq!(result, test_string);
    }

    #[test]
    fn test_string_with_max_len_valid() {
        let test_string = "test".to_string();
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(test_string.len() as u32).unwrap();
        src.extend_from_slice(test_string.as_bytes());
        let result = parse_string_max_len(&mut Cursor::new(src), 10).unwrap();
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

        let result = parse_string_max_len(&mut Cursor::new(src), 10);
        assert!(matches!(result, Err(Error::MaxELemLimit)));
    }

    #[test]
    fn test_read_error() {
        let mut src = Vec::new();
        src.write_u32::<BigEndian>(10).unwrap();
        let result = parse_vector(&mut Cursor::new(src), |s| parse_u8(s));
        assert!(matches!(result, Err(Error::IO(_))));
    }
}
