import { isValidTransportAddr } from "@/views/shared/address-rules";

// 添加好友表单预校验，与后端 crates/p2p-chat friend_add 同口径：
// peerId = base58 解码恰 32 字节（model.rs parse_peer_id）；
// 昵称 = trim 后按字符数 ≤64（validate_nickname）；
// 地址 = TransportAddr 语法（ip/u端口 或 ip/t端口），复用共享规则。
const BASE58_RE = /^[1-9A-HJ-NP-Za-km-z]+$/;
const BASE58_ALPHABET = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const PEER_ID_BYTES = 32;
const MAX_NICKNAME_CHARS = 64;

export type FriendFieldError =
  | "peerIdRequired"
  | "peerIdInvalid"
  | "nicknameTooLong"
  | "addrInvalid";

// base58 大数解码，仅取字节长度用于校验（无需还原内容）。
export function base58ByteLength(value: string): number | null {
  if (value.length === 0 || !BASE58_RE.test(value)) return null;
  const bytes: number[] = [];
  for (const ch of value) {
    let carry = BASE58_ALPHABET.indexOf(ch);
    for (let i = bytes.length - 1; i >= 0; i -= 1) {
      carry += bytes[i]! * 58;
      bytes[i] = carry % 256;
      carry = Math.floor(carry / 256);
    }
    while (carry > 0) {
      bytes.unshift(carry % 256);
      carry = Math.floor(carry / 256);
    }
  }
  let leadingZeros = 0;
  while (value[leadingZeros] === "1") leadingZeros += 1;
  return bytes.length + leadingZeros;
}

export function isValidFriendPeerId(value: string): boolean {
  return base58ByteLength(value) === PEER_ID_BYTES;
}

export function nicknameCharCount(value: string): number {
  return Array.from(value).length;
}

export interface FriendFormErrors {
  peerId?: FriendFieldError;
  nickname?: FriendFieldError;
  addrs: Record<number, FriendFieldError>;
}

export function hasFriendFormErrors(errors: FriendFormErrors): boolean {
  return (
    errors.peerId !== undefined ||
    errors.nickname !== undefined ||
    Object.keys(errors.addrs).length > 0
  );
}

// 空地址行忽略（提交前由调用方过滤），非空行逐条语法校验。
export function validateFriendForm(
  peerId: string,
  nickname: string,
  addrs: string[],
): FriendFormErrors {
  const errors: FriendFormErrors = { addrs: {} };
  if (peerId.trim().length === 0) errors.peerId = "peerIdRequired";
  else if (!isValidFriendPeerId(peerId.trim())) errors.peerId = "peerIdInvalid";
  if (nicknameCharCount(nickname.trim()) > MAX_NICKNAME_CHARS) {
    errors.nickname = "nicknameTooLong";
  }
  addrs.forEach((addr, index) => {
    if (addr.trim().length > 0 && !isValidTransportAddr(addr.trim())) {
      errors.addrs[index] = "addrInvalid";
    }
  });
  return errors;
}
