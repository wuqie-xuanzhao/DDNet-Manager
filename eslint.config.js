import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [".claude/**", ".playwright-mcp/**", "dist/**", "node_modules/**", "src-tauri/**", "tmp/**"]
  },
  js.configs.recommended,
  ...tseslint.configs.strict,
  {
    files: ["src/**/*.{ts,tsx}", "vite.config.ts"],
    languageOptions: {
      ecmaVersion: "latest",
      globals: {
        ...globals.browser,
        ...globals.es2022
      }
    },
    plugins: {
      "react-hooks": reactHooks
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "react-hooks/set-state-in-effect": "off",
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-invalid-void-type": "off",
      "@typescript-eslint/consistent-type-imports": [
        "error",
        {
          prefer: "type-imports",
          fixStyle: "inline-type-imports"
        }
      ]
    }
  }
);
