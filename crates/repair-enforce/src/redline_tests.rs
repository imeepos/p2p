//! 红线判定测试：五条红线逐条直击 + 变形绕过企图（大小写/路径变形）。
//! 独立文件便于红线扫描门禁按 *_tests.rs 约定排除。

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

    mod format_redline {
        use super::*;

        #[test]
        fn hits() {
            assert_eq!(
                check_tool_call(&shell("format c:")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("mkfs.ext4 /dev/sdb")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("diskpart")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("dd if=/dev/zero of=/dev/sda")),
                Some(Redline::FormatDisk)
            );
        }

        #[test]
        fn case_deformation() {
            assert_eq!(
                check_tool_call(&shell("FORMAT C:")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("ForMat c:")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("MKFS.EXT4 /dev/sdb")),
                Some(Redline::FormatDisk)
            );
        }

        #[test]
        fn tool_alias_form() {
            assert_eq!(
                check_tool_call(&shell("format.com c:")),
                Some(Redline::FormatDisk)
            );
            assert_eq!(
                check_tool_call(&shell("low level format d:")),
                Some(Redline::FormatDisk)
            );
        }

        #[test]
        fn safe_words_not_flagged() {
            assert_eq!(check_tool_call(&shell("dir formatted_logs")), None);
            assert_eq!(check_tool_call(&shell("type transformer.log")), None);
        }
    }

    mod credentials_redline {
        use super::*;

        #[test]
        fn path_hits_fs_tools() {
            let cases = [
                ("C:\\Users\\Jane\\.ssh\\id_rsa", "id_rsa"),
                ("C:/Users/Jane/.ssh/id_ed25519", "id_ed25519"),
                ("/etc/shadow", "shadow"),
                ("C:\\Users\\Jane\\.env", ".env"),
                ("C:/Users/Jane/AppData/Roaming/foo/kdbx", "kdbx"),
                ("/Users/Jane/.gnupg/private", "private"),
            ];
            for (p, _name) in cases {
                assert_eq!(
                    check_tool_call(&fs("fs_read", &[("path", p)])),
                    Some(Redline::Credentials),
                    "fs_read {p}"
                );
                assert_eq!(
                    check_tool_call(&fs("fs_write", &[("path", p)])),
                    Some(Redline::Credentials),
                    "fs_write {p}"
                );
            }
        }

        #[test]
        fn path_deformation_bypass() {
            let p = "C:\\Users\\Jane\\.ssh\\id_rsa";
            let variants = [
                p.to_string(),
                "C:/Users/Jane/.ssh/id_rsa".to_string(),
                "c:\\users\\jane\\.ssh\\id_rsa".to_string(),
                "C:\\Users\\Jane\\Desktop\\..\\.ssh\\id_rsa".to_string(),
                "\\\\?\\C:\\Users\\Jane\\.ssh\\id_rsa".to_string(),
                "C:\\Users\\Jane\\..\\Jane\\.ssh\\id_rsa".to_string(),
            ];
            for v in variants {
                assert_eq!(
                    check_tool_call(&fs("fs_read", &[("path", &v)])),
                    Some(Redline::Credentials),
                    "variant {v}"
                );
            }
        }

        #[test]
        fn shell_touching_credentials() {
            assert_eq!(
                check_tool_call(&shell("type C:\\Users\\Jane\\.ssh\\id_rsa")),
                Some(Redline::Credentials)
            );
            assert_eq!(
                check_tool_call(&shell("copy id_rsa backup\\")),
                Some(Redline::Credentials)
            );
            assert_eq!(
                check_tool_call(&shell("TYPE C:\\USERS\\JANE\\.SSH\\ID_RSA")),
                Some(Redline::Credentials)
            );
        }

        #[test]
        fn safe_reads_not_flagged() {
            assert_eq!(
                check_tool_call(&fs(
                    "fs_read",
                    &[("path", "C:/Users/Jane/Documents/report.txt")]
                )),
                None
            );
            assert_eq!(
                check_tool_call(&fs("fs_list", &[("path", "C:/Program Files/App")])),
                None
            );
        }
    }

    mod encrypt_redline {
        use super::*;

        #[test]
        fn hits() {
            let cases = [
                "openssl enc -aes-256-cbc -in secret.txt -out s.enc",
                "gpg --encrypt --recipient alice",
                "age --encrypt -o out.age in.txt",
                "cryptsetup luksFormat /dev/sdb",
                "manage-bde -on C:",
            ];
            for c in cases {
                assert_eq!(
                    check_tool_call(&shell(c)),
                    Some(Redline::EncryptUserFiles),
                    "{c}"
                );
            }
        }

        #[test]
        fn case_deformation() {
            assert_eq!(
                check_tool_call(&shell("OpenSSL ENC -aes256 -in a.txt -out b.enc")),
                Some(Redline::EncryptUserFiles)
            );
            assert_eq!(
                check_tool_call(&shell("GPG --ENCRYPT file")),
                Some(Redline::EncryptUserFiles)
            );
        }

        #[test]
        fn safe_openssl_not_flagged() {
            assert_eq!(check_tool_call(&shell("openssl dgst -sha256 a.txt")), None);
            assert_eq!(check_tool_call(&shell("gpg --verify sig.asc")), None);
        }
    }
}
