//! 批量删除 / 杀毒失效红线 + 正常操作不误伤（与 redline_tests.rs 拆分控行数）。

#[cfg(test)]
mod tests {
    use crate::redline::*;
    use crate::risk::ToolCall;

    fn shell(argv: &str) -> ToolCall {
        ToolCall {
            tool: "shell_exec".to_string(),
            params: vec![("argv".to_string(), argv.to_string())],
        }
    }

    fn fs(tool: &str, params: &[(&str, &str)]) -> ToolCall {
        ToolCall {
            tool: tool.to_string(),
            params: params
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    mod batch_delete_redline {
        use super::*;

        #[test]
        fn multi_path_single_call() {
            assert_eq!(
                check_tool_call(&shell("rm a.txt b.txt")),
                Some(Redline::BatchDelete)
            );
            assert_eq!(
                check_tool_call(&shell("del f1 f2 f3")),
                Some(Redline::BatchDelete)
            );
            assert_eq!(
                check_tool_call(&shell("Remove-Item x y")),
                Some(Redline::BatchDelete)
            );
        }

        #[test]
        fn recursive_user_dir() {
            let cases = [
                "rm -rf C:\\Users\\Jane\\Desktop",
                "rm -rf /home/jane",
                "rm -rf /Users/jane/Documents",
                "rd /s /q D:\\",
                "Remove-Item C:/Users -Recurse",
                "rm -rf ~/Documents",
                "rm -rf /Users",
            ];
            for c in cases {
                assert_eq!(
                    check_tool_call(&shell(c)),
                    Some(Redline::BatchDelete),
                    "{c}"
                );
            }
        }

        #[test]
        fn path_case_deformation() {
            let cases = [
                "RM -RF C:\\USERS\\JANE\\DESKTOP",
                "rm -rf C:/Users/Jane/Documents",
                "rm -rf C:\\Users\\..\\Users\\Jane\\Documents",
                "rm -rf \\\\?\\C:\\Users\\Jane\\Desktop",
                "rM -rF /HoMe/JaNe",
            ];
            for c in cases {
                assert_eq!(
                    check_tool_call(&shell(c)),
                    Some(Redline::BatchDelete),
                    "{c}"
                );
            }
        }

        #[test]
        fn fs_delete_multi_and_recursive() {
            assert_eq!(
                check_tool_call(&fs("fs_delete", &[("paths", "C:/a.txt C:/b.txt")])),
                Some(Redline::BatchDelete)
            );
            assert_eq!(
                check_tool_call(&fs("fs_delete", &[("path", "C:/Users/Jane/Desktop")])),
                None
            );
            assert_eq!(
                check_tool_call(&fs(
                    "fs_delete",
                    &[("path", "C:/Users/Jane/Desktop"), ("recursive", "true"),]
                )),
                Some(Redline::BatchDelete)
            );
        }

        #[test]
        fn legitimate_single_deletes_not_flagged() {
            assert_eq!(check_tool_call(&shell("rm /tmp/build/old.log")), None);
            assert_eq!(check_tool_call(&shell("del C:\\temp\\a.txt")), None);
            assert_eq!(
                check_tool_call(&shell("rm -rf C:\\Users\\Jane\\AppData\\Local\\Temp\\old")),
                None
            );
            assert_eq!(check_tool_call(&shell("rm -rf /tmp/build/*")), None);
        }
    }

    mod antivirus_redline {
        use super::*;

        #[test]
        fn hits() {
            let cases = [
                "sc stop WinDefend",
                "Stop-Service WinDefend",
                "Set-MpPreference -DisableRealtimeMonitoring true",
                "net stop msmpsvc",
                "taskkill /f /im MsMpEng.exe",
                "reg delete HKLM\\SOFTWARE\\Microsoft\\Windows Defender",
            ];
            for c in cases {
                assert_eq!(
                    check_tool_call(&shell(c)),
                    Some(Redline::DisableAntivirus),
                    "{c}"
                );
            }
        }

        #[test]
        fn case_deformation() {
            assert_eq!(
                check_tool_call(&shell("SC STOP WINDEFEND")),
                Some(Redline::DisableAntivirus)
            );
            assert_eq!(
                check_tool_call(&shell("set-mppreference -disablerealtimemonitoring")),
                Some(Redline::DisableAntivirus)
            );
        }

        #[test]
        fn read_queries_not_flagged() {
            assert_eq!(check_tool_call(&shell("Get-MpComputerStatus")), None);
            assert_eq!(check_tool_call(&shell("sc query WinDefend")), None);
        }
    }

    mod no_false_positive {
        use super::*;

        #[test]
        fn benign_shell_commands() {
            for c in [
                "tasklist",
                "whoami",
                "systeminfo",
                "netsh advfirewall show allprofiles",
                "chkdsk C:",
            ] {
                assert_eq!(check_tool_call(&shell(c)), None, "{c}");
            }
        }

        #[test]
        fn benign_fs_calls() {
            assert_eq!(
                check_tool_call(&fs("fs_read", &[("path", "C:/Windows/win.ini")])),
                None
            );
            assert_eq!(
                check_tool_call(&fs(
                    "fs_write",
                    &[
                        ("path", "C:/Users/Jane/Documents/todo.txt"),
                        ("content", "hello"),
                    ]
                )),
                None
            );
            assert_eq!(check_tool_call(&fs("fs_list", &[("path", "/tmp")])), None);
        }

        #[test]
        fn delete_token_not_command_not_flagged() {
            assert_eq!(
                check_tool_call(&shell("echo model summary > out.txt")),
                None
            );
        }
    }
}
