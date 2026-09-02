# Self-Evolving Notes

## 2026-09-02 V 文档整理（docs/organize）

- 哪个坑浪费了最多时间？
  run_code 多行 bash 字符串的 JS 语法错误，一次失败一次重试；改用数组拼接后一次过。
- skill 有没有提前警告我？
  没有。skill 此前只有 Rust 生态教训，没有 run_code 字符串转义类教训（已喂回 lessons.md）。
  另外设计稿与代码不一致（PeerId 推导）靠任务提示才去核对，应默认不信任设计稿（已喂回）。
- 重来一次会怎么做？
  开工前先 glob "crates/**/*.rs" 拿全清单再排阅读顺序，中途不会撞 rendezvous/ 子目录缺失。
