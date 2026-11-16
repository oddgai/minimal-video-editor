# Minimal Video Editor 実装計画

> **エージェントで作業する場合:** この計画に沿って各タスクを実装する。詳細設計と実装をユーザーから委任されているため、追加の設計承認を求めずに進める。

**目標:** macOS向けの軽量な動画編集アプリを作り、無劣化カット、キーボード操作、手動の音ズレ補正を実現する。Windowsにも対応できる構成を維持する。

**構成:** Tauriアプリから小さな素のTypeScript UIを提供する。Rustでローカルファイルの解析、キーフレーム情報のストリーミング、キャンセル可能なFFmpeg書き出しを安全に行う。動画はOSのWebViewで再生し、JavaScriptで復号しない。

**技術:** Tauri 2、Rust、TypeScript、Vite、Nodeテストランナー、FFmpeg/ffprobe。

**仕様:** `docs/specs/2026-11-13-minimal-video-editor-design.md`

## 全体制約

- macOSを優先しつつ、Windowsでも動くパス・プロセス・ショートカット設計にする。
- 再エンコード、JavaScriptでの全ファイル読み込み、常設プロキシ、バックグラウンドの波形・サムネイル生成を行わない。
- 元ファイルおよび既存の出力先を上書きしない。
- 音ズレ補正の単位は秒。正の値は音声を遅らせる。
- 実行時に外部アセットやサービスへ接続しない。

## レビュー時の重点

- 元動画の時刻が0でない場合と可変フレームレート: キーフレームを正規化し、マーカー範囲を制限する。
- 無音動画とWebViewが対応しないコーデック: 編集を利用可能にし、プレビューエラーを分かりやすく表示する。
- Unicode、空白、アポストロフィを含むファイル名: 引数配列でプロセスを起動し、concat用の項目をエスケープする。
- キャンセルと同時書き出し要求: 書き出しは一つに限定し、未完成の出力を残さない。
- 正負両方の音ズレ補正: ストリームデータを保ち、タイミング差を個別に測定する。

### タスク1: 編集モデルとアプリ設定

**対象:** `package.json`、`tsconfig.json`、`vite.config.ts`、`index.html`、`src/editor.ts`、`tests/editor.test.ts`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`src-tauri/capabilities/default.json`。

**インターフェース:** `Range { start: number; end: number }`。`Editor`が残す範囲、マーカーのスナップ、履歴、キーフレーム移動、音声遅延を管理する。

- [x] 中間範囲の削除、削除区間をまたぐ残し方、遅延を含むUndo/Redo、スナップ、不正・空編集、次／前キーフレーム境界のテストを書く。
- [x] `pnpm test`を実行し、実装がないため失敗することを確認する。
- [x] 純粋な編集モデルと最小限のTauri/Vite設定を実装する。
- [x] `pnpm test`と`pnpm check`を実行し、テストと型検査が通ることを確認する。

### タスク2: ネイティブメディアエンジン

**対象:** `src-tauri/src/media.rs`、`src-tauri/src/export.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/main.rs`、`src-tauri/build.rs`、`src-tauri/tests/media_integration.rs`。

**インターフェース:** `probe_media(path) -> MediaInfo`、`scan_keyframes(path) -> Vec<f64>`、進捗イベント付きの`start_export(request)`、`cancel_export()`。ダイアログで入出力を選び、ローカルアセットURLは選択中の動画に限定する。

- [x] Rustの検証・解析・書き出しテストを追加し、実装前に`cargo test`が失敗することを確認する。
- [x] 実行ファイルの検索、安全な非同期コマンド、メタデータ解析、パケット走査、ストリームコピーによる分割・連結・再多重化、キャンセル、進捗、上書き防止の公開処理を実装する。
- [x] 実FFmpeg素材で保持時間、パケットハッシュ不変性、正負の補正、無音素材、特殊なパス、上書き防止、キャンセルを確認する。
- [x] `cargo test`を実行し、単体・結合テストが通ることを確認する。

### タスク3: デスクトップ編集画面

**対象:** `src/main.ts`、`src/styles.css`、`src/bridge.ts`、`src/player.ts`、`src/icons.ts`、`tests/player.test.ts`。

**インターフェース:** 型付きネイティブブリッジで`MediaInfo`と`Range`を受け取り、`ExportRequest`を送信し、ジョブ単位の進捗を監視する。再生中のみ、`video.currentTime - offset`に音声を同期する。

- [x] 同期処理を実装する前に、正負の音ズレ、境界、停止中の再生位置のテストを行う。
- [x] ネイティブの開く／ドロップ、メタデータ、再生、編集可能なマーカー、キーフレームタイムライン、範囲操作と履歴、音ズレ補正、書き出し／キャンセル、アクセシブルなショートカットダイアログ、読み込み・エラー・空状態を備えた日本語UIを作る。
- [x] `pnpm test`、`pnpm check`、`pnpm build`、デスクトップのコンパイルを確認する。

### タスク4: 配布・検証・レビュー

**対象:** 必要に応じて`README.md`、`.github/workflows/ci.yml`、`scripts/verify-media.mjs`。

- [x] セットアップ、起動・ビルド、FFmpeg検索、ショートカット、精度、プレビュー制限を説明する。
- [x] macOS/Windowsの型検査、編集テスト、Rust確認、アプリビルドをCIに追加する。
- [x] 全テスト、フォーマッター、Clippy、リリースビルド、スモークテストを実行し、実測値と環境上の制約を記録する。
- [x] 新たにコードレビューを依頼し、重要な指摘を修正して、実装と実行可能な成果物をワークスペースに残す。

## 検証記録（2026-11-13）

- `pnpm test`: 編集と音声タイミングを扱う11テストが成功。
- `pnpm check` / `pnpm build`: 成功。圧縮前の本番フロントエンドはJavaScript約47 kB、CSS約16 kB。
- `cargo test --manifest-path src-tauri/Cargo.toml --offline`: 単体2件と実FFmpeg結合9件が成功。
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`cargo clippy --manifest-path src-tauri/Cargo.toml --offline --all-targets -- -D warnings`: 成功。
- `pnpm test:ui`: 実UIとモックしたネイティブIPCを使うChromiumテスト2件が成功。入力保護、キーボードボタン操作、編集・履歴、音ズレ補正、書き出し内容、ダイアログ、ブラウザーのみのエラー動作を確認。ネイティブダイアログやWebView復号は対象外。
- `pnpm desktop:build`: macOSアプリをビルド。外部FFmpegを除き4.52 MiB。ファイルシステムサンドボックス外でネイティブプロセス起動をスモークテスト済み。
- 新たなレビューで、セグメントごとのタイムスタンプ原点の不一致、Enterキーによる起動、バックグラウンドで残る古い走査が見つかった。それぞれ回帰テストを追加して修正した。Bフレームを含む`[0,2]+[4,6]`の結合間隔は2.074707秒から2.000秒になった。
- macOS/Windows CI設定を追加。今回の作業ではWindows CIと対話動作は未実行。署名・公証とFFmpeg同梱は最初のローカル開発ビルドの対象外。
