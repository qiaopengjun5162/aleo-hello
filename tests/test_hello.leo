// The 'test_hello' test program.
import hello.aleo;
program test_hello.aleo {
    @test
    @should_fail
    fn test_main_fails() {
        let result: u32 = hello.aleo::main(2u32, 3u32);
        assert_eq(result, 3u32);
    }

    @noupgrade
    constructor() {}
}
