use cuemol_core::{
    lformat, BinInStream, BinOutStream, ByteArray, ElementType, EventManager, LRegExpr, LString,
    LimitedInStream, Matrix3, Matrix4, ProcessManager, Quat, TaskStatus, Vector4,
};
use std::fs;
use std::sync::{Arc, Mutex};

#[test]
fn vector4d_matches_reference_math_and_torsion_cases() {
    let mut value = Vector4::ZERO;
    assert_eq!(value, Vector4::new4(0.0, 0.0, 0.0, 0.0));
    value.set3(1.0, 2.0, 3.0);
    assert_eq!(value, Vector4::new3(1.0, 2.0, 3.0));
    value.set4(1.0, 2.0, 3.0, 4.0);
    assert_eq!(value, Vector4::new4(1.0, 2.0, 3.0, 4.0));

    assert_eq!(
        Vector4::new3(1.0, 0.0, 0.0).cross(Vector4::new3(0.0, 1.0, 0.0)),
        Vector4::new3(0.0, 0.0, 1.0)
    );
    assert!(Vector4::new4(0.0, 0.0, 0.0, 5.0).is_zero3d());
    assert_eq!(Vector4::new3(3.0, 4.0, 0.0).length(), 5.0);
    assert_eq!(
        Vector4::new3(3.0, 0.0, 0.0).normalize().unwrap(),
        Vector4::new3(1.0, 0.0, 0.0)
    );
    assert_eq!(
        Vector4::new4(1.0, 2.0, 3.0, 4.0).dot(Vector4::new4(4.0, 5.0, 6.0, 7.0)),
        60.0
    );
    assert_eq!(
        Vector4::new3(1.0, 2.0, 3.0) + Vector4::new3(4.0, 5.0, 6.0),
        Vector4::new3(5.0, 7.0, 9.0)
    );
    assert_eq!(
        Vector4::new3(4.0, 5.0, 6.0) - Vector4::new3(1.0, 2.0, 3.0),
        Vector4::new3(3.0, 3.0, 3.0)
    );
    assert!(
        (Vector4::new3(1.0, 0.0, 0.0)
            .angle(Vector4::new3(0.0, 1.0, 0.0))
            .unwrap()
            - std::f64::consts::FRAC_PI_2)
            .abs()
            < 1.0e-10
    );
    assert!(
        (Vector4::angle_static(Vector4::new3(1.0, 0.0, 0.0), Vector4::new3(1.0, 0.0, 0.0))
            .unwrap())
        .abs()
            < 1.0e-10
    );

    let i = Vector4::new3(-1.0, 1.0, 0.0);
    let j = Vector4::new3(0.0, 0.0, 0.0);
    let k = Vector4::new3(1.0, 0.0, 0.0);
    let l = Vector4::new3(2.0, 1.0, 0.0);
    assert!(Vector4::torsion(i, j, k, l).unwrap().abs() < 1.0e-10);
    let l = Vector4::new3(2.0, -1.0, 0.0);
    assert!((Vector4::torsion(i, j, k, l).unwrap().abs() - std::f64::consts::PI).abs() < 1.0e-10);
    assert!(Vector4::torsion(j, j, k, k).is_err());

    assert_eq!(Vector4::new3(1.0, 2.0, 3.0).script_string(), "(1,2,3)");
    assert_eq!(
        Vector4::from_script_string("(1, 2, 3, 4)").unwrap(),
        Vector4::new4(1.0, 2.0, 3.0, 4.0)
    );
}

#[test]
fn bytearray_matches_reference_operations() {
    let mut target = ByteArray::new();
    assert_eq!(target.length(), 0);
    assert_eq!(ByteArray::UINT8, 1);
    assert_eq!(ByteArray::INT32, 13);
    assert_eq!(ByteArray::FLOAT32, 21);
    assert_eq!(ByteArray::FLOAT64, 22);

    target.init(ElementType::Uint8, 100);
    assert_eq!(target.length(), 100);
    for index in 0..100 {
        target.set_value(index, 10).unwrap();
        assert_eq!(target.get_value(index).unwrap(), 10);
    }

    target.init_code(ByteArray::INT32, 2).unwrap();
    assert_eq!(target.elem_type(), ElementType::Int32);
    assert_eq!(target.byte_len(), 8);
    assert_eq!(target.length(), 8);
    assert_eq!(target.elem_count(), 2);

    target.init(ElementType::Int32, 100);
    for index in 0..100 {
        target.set_at(index, -123).unwrap();
        assert_eq!(target.get_at(index).unwrap(), -123);
    }

    target.init(ElementType::Float32, 100);
    for index in 0..100 {
        target.set_at_f(index, 12.34).unwrap();
        assert!((target.get_at_f(index).unwrap() - 12.34).abs() < 1.0e-5);
    }

    target.init(ElementType::Float64, 100);
    for index in 0..100 {
        target.set_at_f(index, 12.34).unwrap();
        assert!((target.get_at_f(index).unwrap() - 12.34).abs() < 1.0e-12);
    }
}

#[test]
fn bytearray_typed_copy_api_matches_ndarray_copy_semantics() {
    let uints = ByteArray::copy_from_u8(&[1, 2, 255]);
    assert_eq!(uints.elem_type(), ElementType::Uint8);
    assert_eq!(uints.copy_to_u8().unwrap(), [1, 2, 255]);

    let ints = ByteArray::copy_from_i32(&[-123456, 0, 123456]);
    assert_eq!(ints.elem_type_code(), ByteArray::INT32);
    assert_eq!(ints.copy_to_i32().unwrap(), [-123456, 0, 123456]);

    let floats = ByteArray::copy_from_f32(&[1.25, -2.5]);
    assert_eq!(floats.elem_type_code(), ByteArray::FLOAT32);
    assert_eq!(floats.copy_to_f32().unwrap(), [1.25, -2.5]);

    let doubles = ByteArray::copy_from_f64(&[1.25, -2.5]);
    assert_eq!(doubles.elem_type_code(), ByteArray::FLOAT64);
    assert_eq!(doubles.copy_to_f64().unwrap(), [1.25, -2.5]);
}

#[test]
fn bytearray_rejects_type_mismatched_accessors_like_reference() {
    let mut target = ByteArray::new();

    target.init(ElementType::Float32, 1);
    assert!(target.get_at(0).is_err());
    assert!(target.set_at(0, 1).is_err());
    assert!(target.get_value(0).is_err());

    target.init(ElementType::Int32, 1);
    assert!(target.get_at_f(0).is_err());
    assert!(target.set_at_f(0, 1.0).is_err());
}

#[test]
fn binstream_roundtrips_reference_binary_values() {
    let mut out = BinOutStream::new();
    out.write_int8(42);
    out.write_int8(-1);
    out.write_int16(0x1234);
    out.write_int32(0x12345678);
    out.write_int32(1);
    out.write_int32(2);
    out.write_int32(3);
    out.write_float32(2.75);
    out.write_str("hello");
    out.write_str("");

    let mut input = BinInStream::new(out.into_bytes());
    assert_eq!(input.read_int8().unwrap(), 42);
    assert_eq!(input.read_int8().unwrap(), -1);
    assert_eq!(input.read_int16().unwrap(), 0x1234);
    assert_eq!(input.read_int32().unwrap(), 0x12345678);
    assert_eq!(input.read_int32().unwrap(), 1);
    assert_eq!(input.read_int32().unwrap(), 2);
    assert_eq!(input.read_int32().unwrap(), 3);
    assert!((input.read_float32().unwrap() - 2.75).abs() < f32::EPSILON);
    assert!(input.read_str().unwrap().equals("hello"));
    assert!(input.read_str().unwrap().is_empty());
}

#[test]
fn binstream_fixed_string_swap_and_assert_match_reference() {
    let mut out = BinOutStream::new();
    out.write_fixed_str("hi", 4);
    assert_eq!(out.as_bytes(), b"hi\0\0");

    let mut out = BinOutStream::new();
    out.write_int16(0x0102);
    out.write_int32(0x01020304);
    out.write_int32(99);
    let mut input = BinInStream::new(out.into_bytes());
    assert_eq!(input.get_swap_mode(), BinInStream::MODE_NOOP);
    input.set_swap_mode(BinInStream::MODE_SWAP);
    assert_eq!(input.get_swap_mode(), BinInStream::MODE_SWAP);
    assert_eq!(input.read_int16().unwrap(), 0x0201);
    assert_eq!(input.read_int32().unwrap(), 0x04030201);
    input.set_swap_mode(BinInStream::MODE_NOOP);
    assert!(input.assert_i32(99).unwrap());
}

#[test]
fn limited_in_stream_caps_reads_like_reference_filter() {
    fn drain(stream: &mut LimitedInStream) -> String {
        let mut out = Vec::new();
        loop {
            let value = stream.read_byte();
            if value < 0 {
                break;
            }
            out.push(value as u8);
        }
        String::from_utf8(out).unwrap()
    }

    let mut stream = LimitedInStream::new("abcdef", 100);
    assert_eq!(drain(&mut stream), "abcdef");
    assert!(!stream.ready());
    assert_eq!(stream.read_byte(), -1);

    let mut stream = LimitedInStream::new("abcdef", 6);
    assert_eq!(drain(&mut stream), "abcdef");
    assert!(!stream.ready());

    let mut stream = LimitedInStream::new("abcdefghij", 4);
    assert_eq!(drain(&mut stream), "abcd");
    assert_eq!(stream.read_byte(), -1);

    let mut stream = LimitedInStream::new("0123456789ABCDEF", 5);
    let mut buf = [0; 16];
    assert_eq!(stream.read(&mut buf, 0, 16), 5);
    assert_eq!(&buf[..5], b"01234");
    assert!(stream.read(&mut buf, 0, 8) <= 0);
    assert!(!stream.ready());

    let mut stream = LimitedInStream::new("hello", 0);
    assert!(!stream.ready());
    assert_eq!(stream.read_byte(), -1);
}

#[test]
fn lstring_matches_reference_core_operations() {
    assert!(LString::new().is_empty());
    assert_eq!(LString::from("hello").length(), 5);
    assert!(LString::from(String::from("world")).equals("world"));

    let value = LString::from("hello world");
    assert!(value.substr(6).equals("world"));
    assert!(value.substr_len(0, 5).equals("hello"));
    assert!(value.mid(6).equals("world"));
    assert!(value.mid_len(6, 5).equals("world"));
    assert!(value.left(5).equals("hello"));
    assert!(value.right(5).equals("world"));

    let a = LString::from("abc");
    let b = LString::from("abc");
    let c = LString::from("xyz");
    assert_eq!(a.compare(&b), 0);
    assert!(a.compare(&c) < 0);
    assert!(c.compare(&a) > 0);
    assert!(a.equals(&b));
    assert!(!a.equals(&c));
    assert!(LString::from("Hello").equals_ignore_case("HELLO"));

    assert!(LString::from("Hello World")
        .to_upper_case()
        .equals("HELLO WORLD"));
    assert!(LString::from("Hello World")
        .to_lower_case()
        .equals("hello world"));
}

#[test]
fn lstring_search_convert_split_and_replace_match_reference() {
    let value = LString::from("hello world");
    assert_eq!(value.index_of_char('o'), 4);
    assert_eq!(value.last_index_of_char('o'), 7);
    assert_eq!(value.index_of("world"), 6);
    assert_eq!(value.last_index_of("l"), 9);
    assert_eq!(value.index_one_of("aeiou"), 1);
    assert!(value.starts_with("hello"));
    assert!(value.ends_with("world"));

    assert_eq!(LString::from("42").to_int(), Some(42));
    assert!((LString::from("2.75").to_double().unwrap() - 2.75).abs() < 1.0e-10);
    assert_eq!(LString::from("abc").to_int(), None);
    assert!(lformat!("x={}, y={:.2}", 10, 2.75).starts_with("x=10"));
    assert!(LString::from_int(123).equals("123"));
    assert!(!LString::from_real(2.75, 3).is_empty());

    let values = LString::from("a,b,c").split(',');
    assert_eq!(values.len(), 3);
    assert!(values.first().unwrap().equals("a"));
    assert!(values.last().unwrap().equals("c"));
    assert!(LString::join(",", &values).equals("a,b,c"));

    assert!(LString::from("  hello  ").trim().equals("hello"));
    assert!(LString::from("hello\n").chomp().equals("hello"));
    assert!(LString::from("hello\r\n").chomp().equals("hello"));

    let mut value = LString::from("hello world");
    assert_eq!(value.replace_char('o', 'O'), 2);
    assert!(value.equals("hellO wOrld"));

    let mut value = LString::from("foo bar foo");
    assert_eq!(value.replace_substr("foo", "baz"), 2);
    assert!(value.equals("baz bar baz"));

    assert!((LString::from("hello") + LString::from(" world")).equals("hello world"));
}

#[test]
fn lregexp_matches_reference_basic_and_capture_cases() {
    let mut re = LRegExpr::new();
    assert!(re.to_script_string().equals(""));
    re.set_pattern("(\\d+)");
    assert!(re.get_pattern().equals("(\\d+)"));
    assert!(re.to_script_string().equals("(\\d+)"));
    assert!(re.match_text("123 45").unwrap());
    assert_eq!(re.get_substr_count(), 2);
    assert!(re.get_substr(0).unwrap().equals("123"));
    assert!(re.get_substr(1).unwrap().equals("123"));
    assert!(!re.match_text("abc def").unwrap());

    re.set_pattern("(\\d+)\\s+(\\w+)");
    assert!(re.match_text("123 abc").unwrap());
    assert_eq!(re.get_substr_count(), 3);
    assert!(re.get_substr(0).unwrap().equals("123 abc"));
    assert!(re.get_substr(1).unwrap().equals("123"));
    assert!(re.get_substr(2).unwrap().equals("abc"));

    re.set_pattern("hello");
    assert!(!re.match_text("HELLO world").unwrap());
    assert!(re.match_ignore_case("HELLO world").unwrap());

    re.set_pattern("(unclosed");
    assert!(re.match_text("hello").is_err());

    re.setup(r"^(foo|bar)-(\d*)?$");
    assert!(re.r#match("bar-123").unwrap());
    assert!(re.subject().equals("bar-123"));
    assert_eq!(re.size(), 3);
    assert!(re.at(1).unwrap().equals("bar"));
    assert_eq!(re.to_string(), r"^(foo|bar)-(\d*)?$");
    assert!(!re.r#match("prefix-bar-123").unwrap());
}

#[test]
fn lregexp_matches_reference_pattern_update_and_named_capture_cases() {
    let mut re = LRegExpr::with_pattern("(\\d+)");
    assert!(re.match_text("123").unwrap());

    re.set_pattern("[a-z]+");
    assert!(re.get_pattern().equals("[a-z]+"));
    assert!(re.match_text("abc").unwrap());
    assert!(!re.match_text("123").unwrap());

    re.set_pattern("\\w+@\\w+\\.\\w+");
    assert!(re.match_text("user@example.com").unwrap());
    assert!(!re.match_text("not-an-email").unwrap());

    re.set_pattern("(?P<year>\\d{4})-(?P<month>\\d{2})-(?P<day>\\d{2})");
    assert!(re.match_text("2024-03-15").unwrap());
    assert!(re.get_named_substr("year").unwrap().equals("2024"));
    assert!(re.get_named_substr("month").unwrap().equals("03"));
    assert!(re.get_named_substr("day").unwrap().equals("15"));
}

#[test]
fn event_manager_delegates_events_in_order_like_reference() {
    let mut manager = EventManager::new();
    assert!(manager.is_main_thread());
    manager.message_loop();

    let fired = Arc::new(Mutex::new(Vec::new()));
    for value in [1, 2, 3] {
        let fired = Arc::clone(&fired);
        manager.delegate_event_fire(move || fired.lock().unwrap().push(value));
    }
    assert!(fired.lock().unwrap().is_empty());
    manager.message_loop();
    assert_eq!(*fired.lock().unwrap(), vec![1, 2, 3]);
}

#[test]
fn event_manager_reports_non_main_thread_and_accepts_background_delegation() {
    let manager = Arc::new(Mutex::new(EventManager::new()));
    let manager_for_thread = Arc::clone(&manager);
    let is_main = std::thread::spawn(move || manager_for_thread.lock().unwrap().is_main_thread())
        .join()
        .unwrap();
    assert!(!is_main);

    let fired = Arc::new(Mutex::new(Vec::new()));
    let manager_for_thread = Arc::clone(&manager);
    let fired_for_thread = Arc::clone(&fired);
    std::thread::spawn(move || {
        manager_for_thread
            .lock()
            .unwrap()
            .delegate_event_fire(move || fired_for_thread.lock().unwrap().push(99));
    })
    .join()
    .unwrap();

    assert!(fired.lock().unwrap().is_empty());
    manager.lock().unwrap().message_loop();
    assert_eq!(*fired.lock().unwrap(), vec![99]);
}

#[test]
fn event_manager_timer_queue_matches_reference_progress_and_cancellation() {
    let mut manager = EventManager::new();
    manager.check_timer_queue(0);

    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_timer = Arc::clone(&calls);
    let timer = manager.set_timer(0, 1000, move |t, curr, last| {
        calls_for_timer.lock().unwrap().push((t, curr, last));
        true
    });

    manager.check_timer_queue(500);
    assert_eq!(calls.lock().unwrap().len(), 1);
    assert!((calls.lock().unwrap()[0].0 - 0.5).abs() < 1.0e-9);
    assert!(!calls.lock().unwrap()[0].2);
    assert_eq!(manager.timer_count(), 1);

    manager.check_timer_queue(1500);
    assert_eq!(calls.lock().unwrap().len(), 2);
    assert_eq!(calls.lock().unwrap()[1], (1.0, 1500, true));
    assert_eq!(manager.timer_count(), 0);
    assert!(!manager.remove_timer(timer));

    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_for_timer = Arc::clone(&calls);
    manager.set_timer(0, 1000, move |t, curr, last| {
        calls_for_timer.lock().unwrap().push((t, curr, last));
        false
    });
    manager.check_timer_queue(500);
    manager.check_timer_queue(700);
    assert_eq!(calls.lock().unwrap().len(), 1);

    manager.fini_timer();
    assert_eq!(manager.timer_count(), 0);
}

#[test]
fn process_manager_matches_reference_queue_flow() {
    let mut manager = ProcessManager::new();
    assert_eq!(manager.queue_len(), 0);
    assert_eq!(manager.error_msg(), "");
    assert_eq!(manager.errormsg(), "");

    manager.set_slot_size(1);
    assert_eq!(manager.get_slot_size(), 1);

    let first = manager.queue_task("sleep", "1", "").unwrap();
    let second = manager.queue_task("sleep", "1", "").unwrap();
    assert_ne!(first, second);
    assert!(manager.is_alive(first));
    assert_eq!(manager.task_status(first).unwrap(), TaskStatus::Running);
    assert!(manager.queue_len() > 0);

    manager.kill(first).unwrap();
    assert_eq!(manager.task_status(first).unwrap(), TaskStatus::Ended);
    manager.wait_for_exit(second).unwrap();
    assert_eq!(manager.task_status(second).unwrap(), TaskStatus::Ended);
    assert_eq!(
        manager.done_task_list_json(),
        format!("[{},{}]", first.get(), second.get())
    );
}

#[test]
fn process_manager_captures_output_and_log_file() {
    let log_path = std::env::temp_dir().join(format!("cuemol2-rs-proc-{}.log", std::process::id()));
    let _ = fs::remove_file(&log_path);

    let mut manager = ProcessManager::new();
    manager.set_log_path(&log_path);
    let task = manager.queue_task("printf", "hello", "").unwrap();
    manager.wait_for_exit(task).unwrap();

    assert_eq!(manager.result_output(task).unwrap(), "hello");
    assert_eq!(fs::read_to_string(&log_path).unwrap(), "hello");
    let _ = fs::remove_file(log_path);
}

#[test]
fn matrix4_matches_reference_transform_cases() {
    let mut matrix = Matrix4::IDENTITY;
    matrix.set_at(2, 3, 3.125).unwrap();
    assert_eq!(matrix.get_at(2, 3).unwrap(), 3.125);
    assert_eq!(matrix.ai(10).unwrap(), 3.125);

    let trans = Matrix4::make_trans_mat(Vector4::new3(1.0, 2.0, 3.0));
    let mut origin = Vector4::new3(0.0, 0.0, 0.0);
    trans.xform3d(&mut origin);
    assert_eq!(origin, Vector4::new3(1.0, 2.0, 3.0));

    let inv = trans.invert().unwrap();
    let prod = trans.mul_mat(inv);
    for row in 1..=4 {
        for col in 1..=4 {
            let expected = if row == col { 1.0 } else { 0.0 };
            assert!((prod.get_at(row, col).unwrap() - expected).abs() < 1.0e-10);
        }
    }
}

#[test]
fn matrix4_script_string_matches_lscrmatrix4d_reference() {
    assert_eq!(Matrix4::IDENTITY.to_script_string(), "(identity)");
    assert!(Matrix4::from_script_string("(identity)")
        .unwrap()
        .is_ident());
    assert!(Matrix4::from_script_string("(IDENTITY)")
        .unwrap()
        .is_ident());

    let mut matrix = Matrix4::IDENTITY;
    matrix.set_at(1, 1, 2.0).unwrap();
    matrix.set_at(1, 2, 3.0).unwrap();
    matrix.set_at(2, 3, 5.0).unwrap();
    matrix.set_at(3, 4, 7.0).unwrap();
    let parsed = Matrix4::from_script_string(&matrix.to_script_string()).unwrap();
    assert!(parsed.equals(matrix));

    let values = (1..=16)
        .map(|index| format!("{:.7}", index as f64 * 0.5))
        .collect::<Vec<_>>()
        .join(",");
    let parsed = Matrix4::from_script_string(&format!("({values})")).unwrap();
    for index in 1..=16 {
        assert!((parsed.ai(index).unwrap() - index as f64 * 0.5).abs() < 1.0e-6);
    }

    assert!(Matrix4::from_script_string("(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15)").is_err());
    assert!(Matrix4::from_script_string("(1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,abc)").is_err());
    assert!(Matrix4::from_script_string("1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16").is_err());
}

#[test]
fn quat_and_matrix_rotation_match_reference_clockwise_convention() {
    let quat =
        Quat::from_axis_half_angle(Vector4::new3(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_4)
            .unwrap();
    let matrix = quat.to_rot_matrix();
    let mut value = Vector4::new3(1.0, 0.0, 0.0);
    matrix.xform3d(&mut value);

    assert!(value.x.abs() < 1.0e-10);
    assert!((value.y + 1.0).abs() < 1.0e-10);
    assert!(value.z.abs() < 1.0e-10);

    let identity = Quat::from_scalar_vector(1.0, 0.0, 0.0, 0.0);
    let rotated = identity.rotate_x(90.0).unwrap();
    let mut y_axis = Vector4::new3(0.0, 1.0, 0.0);
    rotated.to_rot_matrix().xform3d(&mut y_axis);
    assert!((y_axis.z + 1.0).abs() < 1.0e-10);
}

#[test]
fn quat_slerp_and_euler_cover_reference_basics() {
    let q = Quat::from_axis_half_angle(Vector4::new3(0.0, 0.0, 1.0), std::f64::consts::FRAC_PI_4)
        .unwrap();
    let same = Quat::slerp(q, q, 0.5).unwrap();
    assert!((same.w - q.w).abs() < 1.0e-10);
    assert!((same.z - q.z).abs() < 1.0e-10);

    let euler = Quat::from_euler(0.0, 0.0, 0.0).to_euler();
    assert!(euler.0.abs() < 1.0e-10);
    assert!(euler.1.abs() < 1.0e-10);
    assert!(euler.2.abs() < 1.0e-10);
}

#[test]
fn matrix_basis_and_quaternion_conversion_match_reference() {
    let e1 = Vector4::new3(0.0, 0.0, 1.0);
    let e2 = Vector4::new3(1.0, 0.0, 0.0);
    let translation = Vector4::new3(4.0, 5.0, 6.0);
    let matrix = Matrix4::make_rot_tran_mat(e1, e2, translation);
    assert_eq!(matrix.col(1).unwrap(), Vector4::new4(1.0, 0.0, 0.0, 0.0));
    assert_eq!(matrix.col(2).unwrap(), Vector4::new4(0.0, 1.0, 0.0, 0.0));
    assert_eq!(matrix.col(3).unwrap(), Vector4::new4(0.0, 0.0, 1.0, 0.0));
    assert_eq!(matrix.get_trans_part(), translation);
    assert_eq!(matrix.get_matrix3d(), Matrix3::IDENTITY);

    for axis in [
        Vector4::new3(1.0, 0.0, 0.0),
        Vector4::new3(0.0, 1.0, 0.0),
        Vector4::new3(0.0, 0.0, 1.0),
        Vector4::new3(1.0, 2.0, 3.0).normalize().unwrap(),
    ] {
        for angle in [0.2, 1.4, 2.8, std::f64::consts::PI] {
            let original = Quat::from_axis_angle(axis, angle).unwrap();
            let rotation = original.to_rot_matrix();
            let converted = Quat::make_from_rot_mat(rotation.get_matrix3d()).unwrap();
            assert!(converted.to_rot_matrix().equals(rotation));
        }
    }
}
