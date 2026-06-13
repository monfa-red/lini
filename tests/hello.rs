#[test]
fn hello_sample_compiles_to_expected_svg() {
    let src = std::fs::read_to_string("samples/hello.lini").expect("read samples/hello.lini");
    let svg = lini::compile_str(&src).expect("compile hello.lini");
    insta::assert_snapshot!(svg);
}
