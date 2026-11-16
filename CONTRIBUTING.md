# 開発ガイド

## 開発環境

バージョンはルートの`mise.toml`で指定し、`mise.lock`で配布物のチェックサムを固定しています。miseを使う場合は設定内容を確認してから`mise trust`、`mise install`を実行します。miseを使わない場合もNode.js 26.13.0、pnpm 12.16.1、Rust 1.99.1、actionlint 1.7.12、ShellCheck 0.11.0を用意してください。

開発ツールのバージョンを変更するときは`mise.toml`と`mise.lock`を一緒に更新します。`mise lock`で全対象プラットフォームのチェックサムを再生成し、差分とバージョンの由来をレビューしてください。`mise.lock`は手で編集しません。

FFmpegとffprobeは現時点でアプリに同梱していません。ローカルのRust統合テストとPlaywright E2Eは実行時に両方を使い、E2Eは`libx264`エンコーダーを使って短いMP4を生成します。

macOSではXcode Command Line Toolsが必要です。LinuxではTauriのGTK/WebKit開発パッケージ、FFmpeg、Chromium用の依存ライブラリが必要です。WindowsではMSVC C++ビルドツールとWebView2が必要です。

```sh
pnpm install --frozen-lockfile
pnpm desktop
```

## 検証

```sh
mise run ci
```

このタスクは次の検証を順番に実行します。GitHub Actionsも同じタスクを実行します。

1. `actionlint .github/workflows/ci.yml` — GitHub Actionsワークフロー検査
2. `pnpm check` — TypeScript型検査
3. `pnpm test` — 編集・プレーヤーの単体テスト
4. `pnpm build` — 本番用フロントエンドビルド
5. `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` — Rust整形確認
6. `cargo test --manifest-path src-tauri/Cargo.toml` — Rust単体・メディア統合テスト
7. `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` — Rust静的検査
8. `pnpm test:ui` — Playwright E2E

Playwrightブラウザーが未導入の場合は、`pnpm exec playwright install chromium`を実行します。CIではUbuntu上でTauri/WebKitとPlaywrightに必要なOSパッケージを先に導入します。

GitHub Actionsの自動検証はUbuntuで行います。macOSのネイティブWebView再生とWindowsの対話操作は、このCIでは確認しません。これらの配布前検証は[公開配布Issue](https://github.com/oddgai/minimal-video-editor/issues/2)で管理します。

## テストの追加・変更

- UIの見た目だけでなく利用者の操作と結果を検証します。複数の操作が連なるE2Eでは`test.step`を使い、操作内容と期待結果をレポートで追えるようにします。
- 関数の状態変換は単体テスト、メディアパケットや実FFmpegとの連携はRust統合テスト、画面から出力までの連携はPlaywright E2Eで検証します。
- E2Eでは`tests/e2e/`のfixture生成を再利用します。実動画の追加、ユーザー固有パスの使用、テストArtifactへの実素材保存は避けます。
- 再現手順や期待結果が明確なバグ報告には、可能なら修正と同じ変更で回帰テストを追加します。

## 設計・レビュー

- コードを変更する前に、該当する`docs/specs/`、`docs/adr/`と関連するテストを確認します。
- 複数案や重要な不確実性がある機能では、実装計画に採用案・代替案、懸念、未決事項と決定時期を記録します。単純な変更に長大な計画は作りません。
- レビューではIssueの目的に合っているか、期待する振る舞いがテスト可能か、実装とテストが同じ契約を説明しているかを確認します。AIによる提案も文脈に照らして人が判断し、理由の分からない変更は取り込みません。
- 変更理由、検証結果、未実施の検証、既知の制約をPull Requestに記載します。

詳細な制約は[AGENTS.md](AGENTS.md)、技術判断は[ADR](docs/adr/README.md)を参照してください。

## 参考資料

- [Design Docで代替案・懸念・未決事項を明確にする](https://www.pospome.work/entry/2026/08/24/223309)
- [AIに実装を任せても、人間が要件と変更内容を理解する](https://zenn.dev/avaintelligence/articles/dont-outsource-understanding-to-ai)
- [Playwright E2Eを手順と期待値が読める形で同じリポジトリに保守する](https://speakerdeck.com/daishukojima/ttf2026-e2e-test-harness)
- [miseでツールバージョンを固定し、CIと開発者環境の手順を合わせる](https://i9wa4.github.io/blog/2026-01-06-mise-pre-commit.html)
