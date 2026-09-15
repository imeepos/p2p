# /rd/audio/1 规范（预留）

状态：draft；自 2026-09-15；归属 crates/rd-wire（planned，仅注册无实现）；符合性：无
（未实现）。

## 1. 概览

远程桌面音频信道：host→viewer 单向二进制帧。随音频波（M7 候选）实现；注册表先行占号，
保证协议 ID 不被其他用途抢注。实现时本文按远端桌面线协议总览
（docs/design/remote-desktop-plan.md §3.4）细化：帧信封沿 /rd/video/1 布局
（magic/ver/seq/ts/codec/payload），codec 初始闭集 `{0 opus_raw, 1 aac_adts}`，
chunked 承载大帧；静音抑制与回声控制属宿主策略。
