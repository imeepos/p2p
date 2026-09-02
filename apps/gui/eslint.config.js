import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "node_modules", "src-tauri", "coverage"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      globals: { ...globals.browser },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-refresh/only-export-components": [
        "error",
        { allowConstantExport: true },
      ],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "error",
    },
  },
  {
    // shadcn 惯例：variants 常量与组件同文件导出
    files: ["src/components/ui/**"],
    rules: { "react-refresh/only-export-components": "off" },
  },
  {
    // hook + context 同文件是反馈/主题层的既定形态，fast-refresh 约束让位于复用
    files: [
      "src/components/feedback/**",
      "src/theme/**",
      "src/components/command-palette/**",
    ],
    rules: { "react-refresh/only-export-components": "off" },
  },
  {
    files: ["**/*.test.{ts,tsx}", "src/test/**"],
    languageOptions: {
      globals: { ...globals.browser },
    },
  },
);