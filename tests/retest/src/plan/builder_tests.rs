// Copyright © 2019-21 Qtrac Ltd. All rights reserved.

#[cfg(test)]
mod tests {
    use crate::plan::{DiffKind, Plan, Test};

    #[test]
    fn empty_plan01() {
        let plan = Plan::new().build();
        assert_eq!(
            plan.rt(),
            r#"[ENV]

"#
        );
    }

    #[test]
    fn empty_plan02() {
        let plan = Plan::new()
            .expected_path(r"E:\results\old")
            .actual_path(r"E:\results\new")
            .build();
        assert_eq!(
            plan.rt(),
            r#"[ENV]
EXPECTED_PATH: E:\results\old
ACTUAL_PATH: E:\results\new

"#
        );
    }

    #[test]
    fn empty_test03() {
        let test = Test::new(r"V:\bin\myapp3.exe").build();
        assert_eq!(
            test.rt(3),
            r#"[3]
APP: V:\bin\myapp3.exe
"#
        );
    }

    #[test]
    fn test04() {
        let test = Test::new(r"V:\bin\myapp4.exe")
            .name("Generic Test 4")
            .args(&["-v", "--format=light"])
            .build();
        assert_eq!(
            test.rt(4),
            r#"[4]
NAME: Generic Test 4
APP: V:\bin\myapp4.exe
     -v
     --format=light
"#
        );
    }

    #[test]
    fn test05() {
        let test = Test::new(r"V:\bin\myapp5.exe")
            .name("Generic Test 5")
            .exit_code(2)
            .args(&["-v", "--format=light"])
            .build();
        assert_eq!(
            test.rt(5),
            r#"[5]
NAME: Generic Test 5
EXITCODE: 2
APP: V:\bin\myapp5.exe
     -v
     --format=light
"#
        );
    }

    #[test]
    fn test06() {
        let test = Test::new(r"V:\bin\myapp6.exe")
            .name("Plain text test 6")
            .exit_code(1)
            .args(&["-q", r"$OUT_PATH\06.txt"])
            .build();
        assert_eq!(
            test.rt(6),
            r#"[6]
NAME: Plain text test 6
EXITCODE: 1
APP: V:\bin\myapp6.exe
     -q
     $OUT_PATH\06.txt
"#
        );
    }

    #[test]
    fn test07() {
        let test = Test::new(r"V:\bin\myapp7.exe")
            .name("Binary test 7")
            .exit_code(1)
            .args(&["-q", r"$OUT_PATH\07.bin"])
            .diff(DiffKind::Binary)
            .build();
        assert_eq!(
            test.rt(7),
            r#"[7]
NAME: Binary test 7
EXITCODE: 1
APP: V:\bin\myapp7.exe
     -q
     $OUT_PATH\07.bin
DIFF: rt-binary
"#
        );
    }

    #[test]
    fn test08() {
        let test = Test::new(r"V:\bin\myapp8.exe")
            .name("Image test 8")
            .args(&[r"$OUT_PATH\08.png"])
            .build();
        assert_eq!(
            test.rt(8),
            r#"[8]
NAME: Image test 8
APP: V:\bin\myapp8.exe
     $OUT_PATH\08.png
"#
        );
    }

    #[test]
    fn test09() {
        let test = Test::new(r"V:\bin\myapp9.exe")
            .name("JSON test 9")
            .args(&[r"$OUT_PATH\09.json"])
            .build();
        assert_eq!(
            test.rt(9),
            r#"[9]
NAME: JSON test 9
APP: V:\bin\myapp9.exe
     $OUT_PATH\09.json
"#
        );
    }

    #[test]
    fn test10() {
        let test = Test::new("$HOME/bin/myapp10")
            .name("External diff test 10")
            .args(&["$OUT_PATH/10.txt"])
            .diff(DiffKind::custom("diff"))
            .diff_args(&["$EXPECTED_PATH/10.txt", "$ACTUAL_PATH/10.txt"])
            .build();
        assert_eq!(
            test.rt(10),
            r#"[10]
NAME: External diff test 10
APP: $HOME/bin/myapp10
     $OUT_PATH/10.txt
DIFF: diff
      $EXPECTED_PATH/10.txt
      $ACTUAL_PATH/10.txt
"#
        );
    }

    #[test]
    fn test11() {
        let test = Test::new("$HOME/bin/myapp11")
            .args(&["-"])
            .name("stdout test 11")
            .stdout("$OUT_PATH/11.txt") // Old-style <= 2
            .build();
        assert_eq!(
            test.rt(11),
            r#"[11]
NAME: stdout test 11
STDOUT: 11.txt
APP: $HOME/bin/myapp11
     -
"#
        );
    }

    #[test]
    fn test12() {
        let test = Test::new("$HOME/bin/myapp12")
            .name("Interactive usage test 12")
            .args(&["-i"])
            .stdin_redirect(b"Some raw bytes")
            .stdout("12.txt") // New-style >= 3
            .build();
        assert_eq!(
            test.rt(12),
            r#"[12]
NAME: Interactive usage test 12
STDIN: ///14 raw bytes///
STDOUT: 12.txt
APP: $HOME/bin/myapp12
     -i
"#
        );
    }

    #[test]
    fn plan13() {
        let plan = Plan::new()
            .expected_path(r"E:\results\old")
            .actual_path(r"E:\results\new")
            .push(
                Test::new(r"V:\bin\myapp1.exe")
                    .name("Generic Test 1")
                    .args(&["-v", "--format=light"])
                    .build(),
            )
            .push(
                Test::new(r"V:\bin\myapp2.exe")
                    .name("Plain text test 2")
                    .exit_code(1)
                    .args(&["-q", r"$OUT_PATH\02.txt"])
                    .build(),
            )
            .push(
                Test::new(r"V:\bin\myapp3.exe")
                    .name("Binary test 3")
                    .exit_code(1)
                    .args(&["-q", r"$OUT_PATH\03.bin"])
                    .diff(DiffKind::Binary)
                    .build(),
            )
            .push(
                Test::new(r"V:\bin\myapp4.exe")
                    .name("Image test 4")
                    .args(&[r"$OUT_PATH\04.png"])
                    .build(),
            )
            .push(
                Test::new(r"V:\bin\myapp5.exe")
                    .name("JSON test 5")
                    .args(&[r"$OUT_PATH\05.json"])
                    .build(),
            )
            .push(
                Test::new("$HOME/bin/myapp6")
                    .name("External diff test 6")
                    .args(&["$OUT_PATH/6.txt"])
                    .diff(DiffKind::custom("diff"))
                    .diff_args(&[
                        "$EXPECTED_PATH/6.txt",
                        "$ACTUAL_PATH/6.txt",
                    ])
                    .build(),
            )
            .push(
                Test::new("$HOME/bin/myapp7")
                    .args(&["-"])
                    .name("stdout test 7")
                    .stdout("$OUT_PATH/7.txt") // Old-style <= 2
                    .build(),
            )
            .push(
                Test::new("$HOME/bin/myapp8")
                    .name("Interactive usage test 8")
                    .args(&["-i"])
                    .stdin_redirect(b"Lots of\nraw\nbytes!")
                    .stdout("8.txt") // New-style >= 3
                    .build(),
            )
            .build();
        assert_eq!(
            plan.rt(),
            r#"[ENV]
EXPECTED_PATH: E:\results\old
ACTUAL_PATH: E:\results\new

[1]
NAME: Generic Test 1
APP: V:\bin\myapp1.exe
     -v
     --format=light

[2]
NAME: Plain text test 2
EXITCODE: 1
APP: V:\bin\myapp2.exe
     -q
     $OUT_PATH\02.txt

[3]
NAME: Binary test 3
EXITCODE: 1
APP: V:\bin\myapp3.exe
     -q
     $OUT_PATH\03.bin
DIFF: rt-binary

[4]
NAME: Image test 4
APP: V:\bin\myapp4.exe
     $OUT_PATH\04.png

[5]
NAME: JSON test 5
APP: V:\bin\myapp5.exe
     $OUT_PATH\05.json

[6]
NAME: External diff test 6
APP: $HOME/bin/myapp6
     $OUT_PATH/6.txt
DIFF: diff
      $EXPECTED_PATH/6.txt
      $ACTUAL_PATH/6.txt

[7]
NAME: stdout test 7
STDOUT: 7.txt
APP: $HOME/bin/myapp7
     -

[8]
NAME: Interactive usage test 8
STDIN: ///18 raw bytes///
STDOUT: 8.txt
APP: $HOME/bin/myapp8
     -i

"#
        );
    }

    #[test]
    fn plan14() {
        let mut planner = Plan::new();
        planner.expected_path(r"E:\results\old");
        planner.actual_path(r"E:\results\new");
        let mut test = Test::new(r"V:\bin\myapp14.exe");
        test.name("Test 14");
        test.args(&["-x", "--output=data.txt"]);
        test.diff(DiffKind::Binary);
        planner.push(test.build());
        let plan = planner.build();
        assert_eq!(
            plan.rt(),
            r#"[ENV]
EXPECTED_PATH: E:\results\old
ACTUAL_PATH: E:\results\new

[1]
NAME: Test 14
APP: V:\bin\myapp14.exe
     -x
     --output=data.txt
DIFF: rt-binary

"#
        );
    }

    #[test]
    fn test15() {
        let test = Test::new("$HOME/bin/myapp12")
            .name("Interactive usage test 12")
            .args(&["-i"])
            .wait(0.3)
            .stdin_redirect(b"Some raw bytes")
            .stdout("12.txt") // New-style >= 3
            .build();
        assert_eq!(
            test.rt(12),
            r#"[12]
NAME: Interactive usage test 12
STDIN: ///14 raw bytes///
STDOUT: 12.txt
WAIT: 0.300
APP: $HOME/bin/myapp12
     -i
"#
        );
    }
}
