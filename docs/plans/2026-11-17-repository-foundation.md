# リポジトリ開発基盤整備 実装計画

> **エージェントで作業する場合:** この計画をタスクごとに実装し、各タスクの検証結果を確認する。

**目的:** 人間とAIが同じ開発手順・品質基準を使い、変更の理由と動作を理解したうえで安全に開発できる基盤を作る。

**構成:** `mise.toml` にローカルとCI共通の検証タスクを定義し、`mise.lock`でツール配布物のチェックサムを固定する。GitHub Actionsは最小権限でそのタスクを実行し、失敗時にPlaywrightのレポートとトレースを保存する。ルートの開発ガイドは日本語で、設計判断とテストの責務、メディア処理の不変条件、AIの変更レビュー方法を説明する。

**技術:** mise、pnpm、Rust/Cargo、GitHub Actions、Playwright、FFmpeg。

**対象:** Issue #3（リポジトリの整備）。

## 全体制約

- Node.js 26.13.0、pnpm 12.16.1、Rust 1.99.1、actionlint 1.7.12、ShellCheck 0.11.0は`mise.toml`を正として維持する。
- パッケージ操作にはpnpmを使う。
- テスト素材はFFmpegで生成し、個人の動画やパスに依存させない。
- `docs/`と開発ガイドは日本語、ルートの`README.md`は英語とする。
- CIからアプリ配布やリリースは行わない。

## 確認観点

- FFmpeg/ffprobeがない場合、Rust統合テストとE2Eのfixture生成が明確に失敗すること。
- Linux runnerのTauri/WebKit依存、FFmpegのH.264エンコーダー、Chromiumの依存がCI内で満たされること。
- 失敗時のPlaywrightレポート・トレースにユーザー動画や個人情報を含めないこと。
- GitHub Actionsに書き込み権限や秘密情報を不要に与えないこと。
- ローカル手順とCIのコマンドが食い違わないこと。

---

### タスク1: 開発ガイドと共通検証タスク

**作成:** `AGENTS.md`、`CONTRIBUTING.md`、本計画書
**変更:** `mise.toml`

- [x] `mise run ci`の順次タスクを追加する。順序はActionlint、フロントエンド型検査、単体テスト、ビルド、Rust整形、Rustテスト、Clippy、Playwright E2Eとする。
- [x] `mise.lock`に全対象プラットフォームの固定ツール配布URLとチェックサムを記録する。
- [x] 開発者向け文書に環境準備、テスト前提、コマンド、fixtureの扱い、変更時に更新する資料を明記する。
- [x] `AGENTS.md`には、既存コードを読んでから編集すること、設計上の不変条件、動作変更と同時にテスト・仕様を更新すること、AI変更を人が説明可能な形でレビューすることを記す。
- [x] `mise tasks ls`でタスク定義を確認する。`mise run ci`一括実行はsandboxによるユーザー領域のRust symlink書き込み拒否で停止したため、各コマンドは個別に実行する。

### タスク2: GitHub Actionsで検証を自動実行

**作成:** `.github/workflows/ci.yml`
**変更:** `mise.toml`（必要なCI条件のみ）

- [x] Pull Request、mainへのpush、手動実行で起動し、contents読み取り権限だけを持つUbuntu jobを作る。
- [x] Node/pnpm/Rustはmiseの既存固定バージョンからインストールする。
- [x] FFmpeg/ffprobe、Tauri Linuxビルド依存、Playwright Chromiumを導入し、`mise run ci`を実行するWorkflowを設定する。GitHub Actions上の初回実行はpush後に確認する。
- [x] Actionsは検証した公式アクションの完全なコミットSHAに固定し、タグをコメントで示す。
- [x] 失敗時のみ、生成されたPlaywright HTMLレポートとトレースを短い保持期間のArtifactとして保存する。
- [x] workflowに`actionlint`を適用し、ローカルでCIと同じ各検証コマンドを実行する。一括の`mise run ci`はsandboxが既存Rust symlinkへの書き込みを拒否したため、各コマンドを個別に実行した。

### タスク3: E2E失敗時の読みやすさ

**変更:** `playwright.config.ts`、`tests/e2e/editor.spec.ts`（必要な場合のみ）

- [x] Playwrightに一覧とHTMLレポートを設定する。トレースは失敗時のみ残す既存設定を維持する。
- [x] 利用者の操作と期待結果が複数段階にまたがるE2Eに、`test.step`で人が読める操作と期待値を付ける。
- [x] `pnpm test:ui`を実行し、15件のE2E成功とHTMLレポート生成を確認する。

## 採らない範囲

- 新しい言語・パッケージマネージャー、独自CIサービス、重いE2E基盤は追加しない。
- リリースビルド・署名・公証、FFmpegバイナリの同梱は別Issueで扱う。
- 全既存テストの大規模な再構成や、未確認のテストを削除する変更は行わない。
