import type zhCN from "./locales/zh-CN";

type JoinPrefix<Prefix extends string, Key extends string> = `${Prefix}.${Key}`;

type FlattenKeys<T> = T extends object
  ? {
      [K in keyof T & string]: T[K] extends object
        ? JoinPrefix<K, FlattenKeys<T[K]>>
        : K;
    }[keyof T & string]
  : never;

export type I18nKey = FlattenKeys<typeof zhCN>;

declare module "i18next" {
  interface CustomTypeOptions {
    resources: {
      translation: typeof zhCN;
    };
  }
}
