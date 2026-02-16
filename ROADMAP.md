# lustc Roadmap

## v0.2.0 — Error Experience ✅

- Source-location-aware error messages (line number, source line display)
- Parser error recovery (report multiple errors instead of stopping at the first)
- Undefined variable/function detection (add a simple name resolution pass)

## v0.3.0 — Language Feature Expansion ✅

- `structure` definitions → Rust `struct` (with `Name.mk` constructor and `Name.field` accessor)
- Constructors with fields (`| cons : Nat → List Nat → List Nat`) — type application in field types
- Tuples `(a, b)` and tuple types `Nat × Nat`, tuple pattern matching, 1-indexed → 0-indexed field access
- `where` clause local definitions (`def f ... where helper := ...`) — desugared to `let`
- String interpolation `s!"..."` → `format!(...)` with embedded expression parsing
- `List` / `Option` type mapping → `Vec<T>` / `Option<T>` with builtin constructors and pattern matching

## v0.4.0 — Type Checker ✅

- Simple type inference / type checking pass
- Type mismatch error reporting (catch errors before code generation)
- Remove unnecessary `.to_string()` and redundant parentheses from generated code

## v0.5.0 — Generated Code Quality ✅

- `cargo fmt`-equivalent formatted output
- Automatic `snake_case` conversion (Lean's `camelCase` function names → Rust convention)
- Remove unnecessary `#[allow(unused)]` annotations
- Dead code detection and elimination

## v0.6.0 — Module System ✅

- `import` / `open` → Rust `mod` with namespace-qualified resolution
- Multi-file input support (recursive module loading, circular import detection)
- `namespace ... end` blocks → Rust `mod` with `pub` declarations
- Namespace resolution (`Foo.bar` → `foo::bar`) and `open` for unqualified access

## v0.7.0 — Stable Release ✅

- Comprehensive test suite (~135 tests)
- CI/CD (GitHub Actions: test / clippy / fmt)
- Distribution via `cargo install lustc`
- Module-level documentation for all source files

---

## Future Roadmap

以下は「厳密なプロダクション利用」を見据えた今後のロードマップです。
現在の v0.7.0 は Lean4 サブセット向けの安定版であり、実行可能コードの変換では十分実用的ですが、
Lean4 全体のカバレッジや運用面の強化が必要です。優先順は利用要件に応じて調整します。

### v0.8.0 — 型システム強化 & match 網羅性チェック

- 多相型（型変数）の推論とジェネリクス生成
- `match` 式の網羅性チェックと不足パターン警告
- 型エラーの文脈情報と修正提案の充実
- 部分適用とカリー化の最適化

### v0.9.0 — エラー体験 & デバッグ支援

- ソースマップ（生成 Rust コード ↔ 元 Lean コード対応）
- エラー位置のカラム情報と複数箇所ハイライト
- 警告オプション（`-W unused`、`-W incomplete_patterns` 等）
- デバッグビルドでのアサーション挿入と実行トレース

### v0.10.0 — 言語機能の拡張（Lean4 カバレッジ拡大）

- 相互再帰データ型と関数の完全サポート
- 型クラス（type class）とインスタンス解決
- 依存型と Pi 型の型チェック・コード生成
- 型レベル自然数（`Fin n`、`Vector α n` 等）のマッピング

### v0.11.0 — パフォーマンス & 最適化

- 大規模コードベースでのコンパイル時間改善（並列パース/型チェック）
- 生成コードのインライン化とループ最適化
- メモリ使用量の削減（AST/型環境の共有化）
- 増分コンパイルとキャッシュ機構
- 型環境のキャッシュとインクリメンタル型チェック

### v0.12.0 — ビルド・ツールチェーン統合

- `cargo` との統合（`build.rs` での呼び出し、feature flags）
- IDE サポート（LSP サーバー、構文ハイライト、定義ジャンプ）
- Bazel/Gradle 等外部ビルドシステム向けプラグイン
- プリプロセッサ/条件コンパイルのサポート

### v0.13.0 — テスト・品質基盤の強化

- ファジングとプロパティベーステストの導入
- パフォーマンス回帰テスト（ベンチマーク CI）
- PR ごとの生成コード diff チェック
- ユーザーコードの実行例ギャラリー

### v0.14.0 — セキュリティ & 信頼性

- 未検証入力に対するパーサの頑健性（DoS 耐性）
- 生成コードの安全性検証（未定義動作の排除）
- 依存クレートの脆弱性スキャンと自動アップデート
- FFI や unsafe ブロックの明示的検査・警告

### v0.15.0 — 互換性 & バージョニング

- Lean4 バージョンごとの構文差分の吸収
- 生成 Rust コードの MSRV（最小サポート Rust バージョン）指定
- LTS リリースと破壊的変更の管理方針
- 移行ガイドと自動移行ツール

### v1.0.0 — プロダクション対応

- API ドキュメントの自動生成と例示
- コンパイルメトリクスの収集・ダッシュボード
- クラッシュレポートの自動収集（オプトイン）
- リリースプロセスの自動化（changelog、semver）
- 各プラットフォーム（Linux/macOS/Windows）でのバイナリ配布
- コンテナイメージ（Docker）の公式提供
- Nightly リリースチャネル
- サポートポリシーと SLA 文書化

### 長期目標（v1.x+）

- タクティクスと証明項のサポート（定理証明用途の解放）
- メタプログラミング機能（`macro`、`syntax`、`elab`）
- Lean4 標準ライブラリの部分的マッピング
