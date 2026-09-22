import { IpcError } from "./ipc-error";
import { useSyncExternalStore } from "react";
import { productMessages } from "./i18n-product";
import { backupMessages } from "./i18n-backup";
import { uiMessages } from "./i18n-ui";
import { nextMessages } from "./i18n-next";

export const languages = {
  "zh-CN": "简体中文",
  en: "English",
  ja: "日本語",
  ko: "한국어",
} as const;
export type Language = keyof typeof languages;
// The four columns share one typed key set. User-authored names and notes are never translated.
const messages = {
  ...nextMessages,
  ...productMessages,
  ...backupMessages,
  organizeSession: [
    "整理「{title}」",
    "Organize {title}",
    "「{title}」を整理",
    "「{title}」 정리",
  ],
  renameSession: [
    "重命名场次",
    "Rename session",
    "セッション名を変更",
    "세션 이름 변경",
  ],
  mergeSession: [
    "合并到其他场次",
    "Merge into another session",
    "別のセッションに統合",
    "다른 세션으로 병합",
  ],
  splitSession: [
    "拆分场次",
    "Split session",
    "セッションを分割",
    "세션 나누기",
  ],
  sessionName: ["场次名称", "Session name", "セッション名", "세션 이름"],
  sessionActive: ["进行中", "Recording", "記録中", "기록 중"],
  sessionArchived: ["已归档", "Archived", "保存済み", "보관됨"],
  sessionGames: [
    "{count} 场对局",
    "{count} matches",
    "{count} 試合",
    "{count}경기",
  ],
  sessionConfirm: ["保存更改", "Save changes", "変更を保存", "변경 사항 저장"],
  sessionSaving: ["正在保存…", "Saving…", "保存中…", "저장 중…"],
  selectSession: [
    "选择归入的场次",
    "Choose a destination",
    "保存先を選択",
    "저장할 세션 선택",
  ],
  splitDefault: [
    "拆分的场次",
    "Split session",
    "分割したセッション",
    "나눈 세션",
  ],
  splitBoundary: [
    "从哪个账号段开始拆分",
    "Start the new session at",
    "新しいセッションの開始区間",
    "새 세션을 시작할 계정 구간",
  ],
  segmentLabel: [
    "第 {number} 段 · {name} · {count} 局",
    "Segment {number} · {name} · {count} matches",
    "区間 {number} · {name} · {count} 試合",
    "{number}번째 구간 · {name} · {count}경기",
  ],
  mergeSummary: [
    "合并后保留「{title}」，共 {count} 场对局。",
    "Keep “{title}” with {count} matches in total.",
    "「{title}」に統合し、合計 {count} 試合になります。",
    "「{title}」에 총 {count}경기를 보관합니다.",
  ],
  splitSummary: [
    "此账号段及之后的 {moving} 场对局归入新场次，原场次保留 {remaining} 场。",
    "Move this segment and all following ones ({moving} matches) to a new session; keep {remaining} in the original.",
    "この区間以降の {moving} 試合を新しいセッションへ移し、元には {remaining} 試合を残します。",
    "이 구간부터 {moving}경기를 새 세션으로 옮기고 기존 세션에는 {remaining}경기를 남깁니다.",
  ],
  mergeRecording: [
    "合并后的场次继续记录，空闲计时保持不变。",
    "Recording continues in the merged session without resetting the idle timer.",
    "統合後も記録を継続し、待機時間はリセットしません。",
    "병합된 세션에서 기록을 계속하며 유휴 시간은 초기화하지 않습니다.",
  ],
  splitRecording: [
    "新场次继续记录，前半场归档。",
    "Recording continues in the new session; the earlier part is archived.",
    "新しいセッションで記録を継続し、前半を保存します。",
    "새 세션에서 기록을 계속하고 앞부분은 보관합니다.",
  ],
  sessionKeepFacts: [
    "账号、自动记录与补录来源、笔记和玩家标记都会保留。",
    "Accounts, recording sources, notes and player labels are preserved.",
    "アカウント・記録元・ノート・プレイヤーの印は保持されます。",
    "계정, 자동 기록 및 수동 추가 출처, 메모와 플레이어 표시는 유지됩니다.",
  ],
  sessionTitleError: [
    "请输入 1–80 个字符的场次名称。",
    "Enter a session name of 1–80 characters.",
    "1〜80 文字の名前を入力してください。",
    "1~80자의 세션 이름을 입력하세요.",
  ],
  sessionChanged: [
    "场次内容已变化，请刷新后重新选择。",
    "The session changed. Refresh and choose again.",
    "セッションが変更されました。更新して選び直してください。",
    "세션이 변경되었습니다. 새로고침 후 다시 선택하세요.",
  ],
  sessionSame: [
    "请选择另一个场次。",
    "Choose another session.",
    "別のセッションを選択してください。",
    "다른 세션을 선택하세요.",
  ],
  sessionActivePrefix: [
    "最近记录的账号段仍在前半场，请调整分界，或结束场次后再拆分。",
    "The latest recorded segment is in the earlier part. Change the boundary or end the session first.",
    "最新の記録区間が前半にあります。分割位置を変更するか、セッションを終了してください。",
    "최근 기록 구간이 앞부분에 있습니다. 경계를 바꾸거나 세션을 종료하세요.",
  ],
  sessionAlready: [
    "这场对局已归入其他场次，请刷新查看；如需整理，可使用场次合并或拆分。",
    "This match already belongs to another session. Refresh to view it, then merge or split sessions if needed.",
    "この試合は別のセッションにあります。更新後、必要に応じて統合・分割してください。",
    "이미 다른 세션에 있는 경기입니다. 새로고침 후 필요하면 세션을 병합하거나 나누세요.",
  ],
  backfillNewHint: [
    "按对局时间新建一个已归档场次；之后可在「我的游玩」中命名或合并。",
    "Create an archived session dated to this match. Rename or merge it in My Play later.",
    "試合日時で保存済みセッションを作成します。後から名前の変更や統合ができます。",
    "경기 시간을 기준으로 보관 세션을 만듭니다. 내 플레이에서 이름을 바꾸거나 병합할 수 있습니다.",
  ],
  backfillExistingHint: [
    "补录到「{title}」，补录后共 {count} 场对局。",
    "Add to “{title}” for a total of {count} matches.",
    "「{title}」に追加し、合計 {count} 試合になります。",
    "「{title}」에 추가하면 총 {count}경기가 됩니다.",
  ],
  backfillAlreadyHint: [
    "已归入「{title}」，此次仅确认补录来源。",
    "Already in “{title}”; this confirms manual attribution.",
    "「{title}」に登録済みです。手動追加の記録のみ確認します。",
    "이미 「{title}」에 있습니다. 수동 추가 출처만 확인합니다.",
  ],
  dismiss: ["关闭提示", "Dismiss", "閉じる", "알림 닫기"],
  markReviewed: [
    "标记已复盘",
    "Mark reviewed",
    "確認済みにする",
    "복기 완료로 표시",
  ],
  reviewed: ["已复盘", "Reviewed", "確認済み", "복기 완료"],
  search: ["战绩查询", "Match history", "戦績検索", "전적 검색"],
  searchHint: [
    "找到一场值得复盘的对局。",
    "Find a match worth reviewing.",
    "振り返りたい試合を見つけましょう。",
    "복기할 경기를 찾아보세요.",
  ],
  current: ["当前账号", "Current account", "現在のアカウント", "현재 계정"],
  region: ["大区", "Region", "地域", "서버"],
  riotId: ["完整 Riot ID", "Full Riot ID", "完全な Riot ID", "전체 Riot ID"],
  find: ["查询", "Search", "検索", "검색"],
  enterId: ["名称#标签", "Name#TAG", "名前#タグ", "이름#태그"],
  regionHint: [
    "可尝试查询同一运营商下已配置的大区；腾讯服与 Riot 服不互通。跨区可用性由目标服务决定，空结果不代表玩家不存在。",
    "Try configured regions within the same operator. Tencent and Riot access is separate. Cross-region availability depends on the service; empty history does not mean the player does not exist.",
    "同じ運営元の設定済み地域を検索できます。Tencent と Riot は相互検索できません。地域間の利用可否はサービス次第で、履歴が空でもプレイヤーが存在しないとは限りません。",
    "같은 운영사의 설정된 지역을 조회할 수 있습니다. Tencent와 Riot은 상호 조회할 수 없습니다. 교차 조회 가능 여부는 서비스에 따라 다르며, 빈 전적이 플레이어의 부재를 뜻하지는 않습니다.",
  ],
  switchClient: [
    "当前会话不可查询",
    "Unavailable with this session",
    "現在のセッションでは検索不可",
    "현재 세션에서 조회 불가",
  ],
  loading: [
    "正在读取战绩…",
    "Loading matches…",
    "戦績を読み込み中…",
    "전적 불러오는 중…",
  ],
  emptySearch: [
    "按大区和 Riot ID 查找玩家",
    "Search by region and Riot ID",
    "地域と Riot ID で検索",
    "서버와 Riot ID로 검색",
  ],
  emptyHint: [
    "查看详情、下载录像和留下笔记，都不需要补录为本人游玩。",
    "Review, download and take notes without adding matches to your play history.",
    "自分のプレイ記録に追加せずに、詳細・リプレイ・ノートを利用できます。",
    "내 플레이 기록에 추가하지 않아도 상세 보기, 리플레이 다운로드, 메모가 가능합니다.",
  ],
  noMatches: [
    "没有返回对局",
    "No matches returned",
    "試合が見つかりません",
    "반환된 경기가 없습니다",
  ],
  noMatchesHint: [
    "核对大区和玩家名称，或查询其他玩家。",
    "Check the region and name, or search for another player.",
    "地域と名前を確認するか、別のプレイヤーを検索してください。",
    "서버와 이름을 확인하거나 다른 플레이어를 검색하세요.",
  ],
  win: ["胜利", "Victory", "勝利", "승리"],
  loss: ["失败", "Defeat", "敗北", "패배"],
  pending: ["未结算", "Pending", "未確定", "결과 대기"],
  review: ["复盘", "Review", "振り返る", "복기"],
  backfill: [
    "补录对局",
    "Add to my play",
    "自分の記録に追加",
    "내 플레이에 추가",
  ],
  recorded: ["已归档", "Recorded", "記録済み", "기록됨"],
  imported: ["已补录", "Added to my play", "追加済み", "추가됨"],
  previous: ["上一页", "Previous", "前へ", "이전"],
  next: ["下一页", "Next", "次へ", "다음"],
  page: ["第 {page} 页", "Page {page}", "{page} ページ", "{page} 페이지"],
  retry: ["重试", "Retry", "再試行", "다시 시도"],
  cancel: ["取消", "Cancel", "キャンセル", "취소"],
  back: ["返回列表", "Back to list", "一覧に戻る", "목록으로"],
  addTitle: [
    "补录到我的游玩",
    "Add to my play history",
    "自分のプレイ記録に追加",
    "내 플레이 기록에 추가",
  ],
  addHint: [
    "确认你使用「{name}」参与了这场对局，并选择归入的场次。",
    "Confirm you played this match as {name}, then choose a session.",
    "「{name}」として参加した試合であることを確認し、セッションを選んでください。",
    "이 경기를 「{name}」 계정으로 플레이했는지 확인하고 세션을 선택하세요.",
  ],
  addNote: [
    "补录后可在“我的游玩”中查看，也可随时撤销。",
    "Find it in My play. You can undo this later.",
    "「自分のプレイ」で確認でき、後から取り消せます。",
    "내 플레이에서 확인할 수 있으며 나중에 취소할 수 있습니다.",
  ],
  session: ["归入场次", "Session", "セッション", "세션"],
  newSession: [
    "新建补录场次",
    "New backfill session",
    "新しい追加セッション",
    "새 추가 세션",
  ],
  confirmAdd: ["确认补录", "Confirm", "追加する", "추가 확인"],
  added: [
    "已补录到我的游玩。",
    "Added to My play.",
    "自分のプレイに追加しました。",
    "내 플레이에 추가했습니다.",
  ],
  library: [
    "录像与笔记",
    "Replays & notes",
    "リプレイとノート",
    "리플레이 및 메모",
  ],
  libraryHint: [
    "继续复盘看过的对局，保留值得学习的细节。",
    "Return to your matches and keep what you learn.",
    "試合を振り返り、学んだことを残しましょう。",
    "경기를 다시 보고 배운 내용을 남기세요.",
  ],
  recent: ["最近查看", "Recently viewed", "最近見た試合", "최근 조회"],
  saved: ["已收藏", "Saved", "保存済み", "즐겨찾기"],
  downloaded: ["已下载", "Downloaded", "ダウンロード済み", "다운로드됨"],
  notes: ["笔记", "Notes", "ノート", "메모"],
  all: ["全部", "All", "すべて", "전체"],
  save: ["收藏对局", "Save match", "試合を保存", "경기 즐겨찾기"],
  unsave: ["取消收藏", "Unsave", "保存を解除", "즐겨찾기 해제"],
  refresh: ["刷新数据", "Refresh data", "データを更新", "데이터 새로 고침"],
  exportNotes: [
    "导出笔记",
    "Export notes",
    "ノートを書き出す",
    "메모 내보내기",
  ],
  noLibrary: [
    "还没有复盘资料",
    "No review material yet",
    "振り返り資料はまだありません",
    "아직 복기 자료가 없습니다",
  ],
  noLibraryHint: [
    "查询并打开一场对局，即可保留详情和笔记。",
    "Search for a match and open it to keep its details and notes.",
    "試合を検索して開くと、詳細やノートを残せます。",
    "경기를 검색하고 열어 상세 정보와 메모를 보관하세요.",
  ],
  noFilter: [
    "没有符合条件的对局",
    "No matching games",
    "該当する試合はありません",
    "조건에 맞는 경기가 없습니다",
  ],
  searchNotes: [
    "搜索笔记或标签",
    "Search notes or tags",
    "ノートやタグを検索",
    "메모 또는 태그 검색",
  ],
  download: [
    "下载录像",
    "Download replay",
    "リプレイをダウンロード",
    "리플레이 다운로드",
  ],
  queued: ["等待下载", "Queued", "待機中", "대기 중"],
  downloading: ["正在下载", "Downloading", "ダウンロード中", "다운로드 중"],
  validating: ["正在校验", "Validating", "検証中", "검증 중"],
  ready: ["录像已保存", "Replay saved", "リプレイ保存済み", "리플레이 저장됨"],
  missing: [
    "文件已移走，可重新下载",
    "File moved; download again",
    "ファイルが見つかりません。再ダウンロードできます",
    "파일이 이동되었습니다. 다시 다운로드하세요",
  ],
  open: ["播放录像", "Play replay", "リプレイを再生", "리플레이 재생"],
  reveal: [
    "打开所在文件夹",
    "Show in folder",
    "フォルダーで表示",
    "폴더에서 보기",
  ],
  opened: [
    "已请求客户端打开录像。",
    "Replay launch requested.",
    "リプレイの起動を要求しました。",
    "리플레이 실행을 요청했습니다.",
  ],
  more: ["更多操作", "More actions", "その他の操作", "추가 작업"],
  timelineFormatError: [
    "客户端返回的时间线格式不符合预期。",
    "The timeline response has an unexpected format.",
    "タイムラインの応答形式が想定と異なります。",
    "타임라인 응답 형식이 예상과 다릅니다.",
  ],
  timelineMissing: [
    "时间线暂不可用，仍可查看战绩和记笔记。",
    "Timeline unavailable. Match stats and notes are still available.",
    "タイムラインは利用できませんが、戦績とノートは利用できます。",
    "타임라인을 불러올 수 없습니다. 전적과 메모는 이용할 수 있습니다.",
  ],
  unsupportedMode: [
    "该模式暂不支持完整复盘布局，录像与笔记仍可使用。",
    "The full review layout does not support this mode yet. Replays and notes remain available.",
    "このモードの詳細表示は未対応です。リプレイとノートは利用できます。",
    "이 모드는 상세 복기 화면을 아직 지원하지 않습니다. 리플레이와 메모는 이용할 수 있습니다.",
  ],
  language: [
    "语言（预览）",
    "Language (preview)",
    "言語（プレビュー）",
    "언어 (미리 보기)",
  ],
  languageHint: [
    "应用于本地工作区。昵称与自己填写的笔记保持原文。",
    "Applies to the local workspace. Names and your notes keep their original text.",
    "ローカル作業領域に適用します。名前と手入力のノートは原文を保持します。",
    "로컬 작업 공간에 적용됩니다. 이름과 직접 작성한 메모는 원문을 유지합니다.",
  ],
  networkError: [
    "连接失败，请检查网络后重试。",
    "Connection failed. Check your network and retry.",
    "接続に失敗しました。ネットワークを確認して再試行してください。",
    "연결에 실패했습니다. 네트워크를 확인하고 다시 시도하세요.",
  ],
  loginError: [
    "请打开并登录对应的游戏客户端。",
    "Open and sign in to the matching game client.",
    "対応するゲームクライアントを開いてログインしてください。",
    "해당 게임 클라이언트를 열고 로그인하세요.",
  ],
  clientDataPending: [
    "客户端暂未提供这项数据，请稍后重试。",
    "The client has not provided this data yet. Try again later.",
    "クライアントからまだデータが提供されていません。後で再試行してください。",
    "클라이언트에서 아직 데이터를 제공하지 않았습니다. 나중에 다시 시도하세요.",
  ],
  tokenError: [
    "登录状态尚未就绪，请稍后重试。",
    "Sign-in is not ready. Try again shortly.",
    "ログインの準備ができていません。しばらくして再試行してください。",
    "로그인 준비 중입니다. 잠시 후 다시 시도하세요.",
  ],
  notFoundError: [
    "未找到该玩家，请核对大区与完整 Riot ID。",
    "Player not found. Check the region and full Riot ID.",
    "プレイヤーが見つかりません。地域と完全な Riot ID を確認してください。",
    "플레이어를 찾을 수 없습니다. 서버와 전체 Riot ID를 확인하세요.",
  ],
  replayNotFound: [
    "服务器没有提供该录像，可能已过期或尚未生成。",
    "The server has no replay available. It may have expired or not be ready.",
    "サーバーにリプレイがありません。期限切れか、まだ生成されていない可能性があります。",
    "서버에 리플레이가 없습니다. 만료되었거나 아직 생성되지 않았을 수 있습니다.",
  ],
  rateError: [
    "查询过于频繁，请稍后重试。",
    "Too many requests. Try again later.",
    "リクエストが多すぎます。後で再試行してください。",
    "요청이 너무 많습니다. 나중에 다시 시도하세요.",
  ],
  regionError: [
    "当前客户端不支持此大区，请切换对应客户端后查询。",
    "Switch to a client that supports this region.",
    "この地域に対応するクライアントに切り替えてください。",
    "이 서버를 지원하는 클라이언트로 전환하세요.",
  ],
  operatorError: [
    "腾讯服与 Riot 服的查询权限不互通，请登录目标运营商的游戏客户端。",
    "Tencent and Riot access is separate. Sign in to a game client for the target operator.",
    "Tencent と Riot は相互検索できません。対象の運営元のゲームクライアントにログインしてください。",
    "Tencent와 Riot은 상호 조회할 수 없습니다. 대상 운영사의 게임 클라이언트에 로그인하세요.",
  ],
  routeError: [
    "当前会话或目标大区缺少已配置的数据路由，暂不能跨区查询或下载；当前大区战绩仍可通过客户端查询。",
    "A configured data route is missing for this session or target region. Cross-region queries or downloads are unavailable; local client history may still work.",
    "現在のセッションまたは対象地域のデータ経路が未設定です。地域間検索・ダウンロードは利用できませんが、現在の地域の履歴はクライアント経由で取得できる場合があります。",
    "현재 세션 또는 대상 지역의 데이터 경로가 설정되지 않았습니다. 교차 조회나 다운로드는 불가능하지만 현재 지역 전적은 클라이언트로 조회할 수 있습니다.",
  ],
  accessDeniedError: [
    "目标服务拒绝了当前凭据的访问。可尝试登录目标大区后重试；这不表示玩家或对局不存在。",
    "The target service denied access with these credentials. Try signing in to the target region; this does not mean the player or match is missing.",
    "対象サービスが現在の認証情報でのアクセスを拒否しました。対象地域にログインして再試行してください。プレイヤーや試合が存在しないという意味ではありません。",
    "대상 서비스가 현재 인증 정보의 접근을 거부했습니다. 대상 지역에 로그인한 뒤 다시 시도하세요. 플레이어나 경기가 없다는 뜻은 아닙니다.",
  ],
  gatewayDataMissing: [
    "目标服务未提供这项数据，可能已过期或接口已变更；这不证明玩家不存在。",
    "The target service did not provide this data. It may have expired or the endpoint may have changed; this does not prove the player is missing.",
    "対象サービスからデータが提供されませんでした。期限切れや API の変更の可能性があり、プレイヤーの不在を示すものではありません。",
    "대상 서비스에서 데이터를 제공하지 않았습니다. 만료되었거나 API가 변경되었을 수 있으며, 플레이어가 없다는 증거는 아닙니다.",
  ],
  identityError: [
    "数据身份核对失败，请重新查询。",
    "Identity could not be verified. Search again.",
    "データの識別情報を確認できませんでした。再検索してください。",
    "데이터의 신원을 확인할 수 없습니다. 다시 검색하세요.",
  ],
  storageError: [
    "本地文件或归档无法写入，请检查磁盘空间和目录权限。",
    "Could not write local data. Check disk space and folder permissions.",
    "ローカルデータを書き込めません。空き容量と権限を確認してください。",
    "로컬 데이터를 저장할 수 없습니다. 디스크 공간과 폴더 권한을 확인하세요.",
  ],
  versionError: [
    "录像与当前客户端版本不兼容，文件已保留。",
    "Replay and client versions are incompatible. The file is kept.",
    "リプレイとクライアントのバージョンが非対応です。ファイルは保持されています。",
    "리플레이와 클라이언트 버전이 호환되지 않습니다. 파일은 보관됩니다.",
  ],
  installError: [
    "未找到可用的游戏安装。请打开对应游戏客户端后重试，无需登录账号。",
    "No usable game installation was found. Open the matching game client and retry; signing in is not required.",
    "利用可能なゲームが見つかりません。対応するクライアントを開いて再試行してください。ログインは不要です。",
    "사용 가능한 게임 설치를 찾지 못했습니다. 해당 클라이언트를 열고 다시 시도하세요. 로그인은 필요하지 않습니다.",
  ],
  installAmbiguous: [
    "找到多份兼容的游戏安装，请打开要使用的游戏客户端后重试。",
    "Several compatible installations were found. Open the game client you want to use and retry.",
    "互換性のあるゲームが複数見つかりました。使用するクライアントを開いて再試行してください。",
    "호환되는 게임 설치가 여러 개 있습니다. 사용할 클라이언트를 열고 다시 시도하세요.",
  ],
  vanguardError: [
    "Vanguard 正在运行，请通过官方客户端播放此 Riot 录像。",
    "Vanguard is running. Play this Riot replay through the official client.",
    "Vanguard が動作中です。公式クライアントからこの Riot リプレイを再生してください。",
    "Vanguard가 실행 중입니다. 공식 클라이언트에서 이 Riot 리플레이를 재생하세요.",
  ],
  protectionError: [
    "无法确认 Riot 客户端的运行条件，请通过官方客户端播放。",
    "Riot playback requirements could not be checked. Use the official client to play.",
    "Riot の再生条件を確認できません。公式クライアントから再生してください。",
    "Riot 재생 조건을 확인할 수 없습니다. 공식 클라이언트에서 재생하세요.",
  ],
  openError: [
    "游戏未能启动，请检查安装是否完整后重试。",
    "The game could not start. Check the installation and retry.",
    "ゲームを起動できませんでした。インストールを確認して再試行してください。",
    "게임을 시작하지 못했습니다. 설치 상태를 확인하고 다시 시도하세요.",
  ],
  busyError: [
    "客户端正在对局、排队、播放录像或更新，请结束后再试。",
    "The client is busy playing, queueing or updating. Try again afterward.",
    "プレイ・キュー・再生・更新中です。終了後に再試行してください。",
    "클라이언트에서 게임, 대기열, 재생 또는 업데이트가 진행 중입니다. 종료 후 다시 시도하세요.",
  ],
  fileError: [
    "录像文件不完整或无法校验，请重新下载。",
    "Replay is incomplete or invalid. Download it again.",
    "リプレイが不完全か無効です。再ダウンロードしてください。",
    "리플레이가 불완전하거나 유효하지 않습니다. 다시 다운로드하세요.",
  ],
  preparing: [
    "游戏客户端正在准备录像，完成后再点击播放。",
    "The game client is preparing the replay. Press Play again when ready.",
    "クライアントがリプレイを準備中です。完了後に再生してください。",
    "게임 클라이언트가 리플레이를 준비 중입니다. 완료 후 다시 재생하세요.",
  ],
  generalError: [
    "操作未完成，请重试。",
    "The action could not be completed. Try again.",
    "操作を完了できませんでした。再試行してください。",
    "작업을 완료하지 못했습니다. 다시 시도하세요.",
  ],
} as const satisfies Record<string, readonly [string, string, string, string]>;
type Key = keyof typeof messages;
const index: Record<Language, number> = { "zh-CN": 0, en: 1, ja: 2, ko: 3 };
let locale: Language = "zh-CN";
try {
  const saved = localStorage.getItem("league-replay:language");
  if (saved && saved in languages) locale = saved as Language;
} catch {
  /* Storage may be unavailable in previews. */
}
const listeners = new Set<() => void>();
if (typeof document !== "undefined") document.documentElement.lang = locale;
export const getLanguage = () => locale;
export function ui(
  source: string,
  values: Record<string, string | number> = {},
) {
  const entry = (uiMessages as Record<string, readonly string[]>)[source];
  const text = locale === "zh-CN" || !entry ? source : entry[index[locale] - 1];
  return text.replace(/\{(\w+)\}/g, (_, name: string) =>
    String(values[name] ?? ""),
  );
}
export function setLanguage(value: Language) {
  locale = value;
  if (typeof document !== "undefined") document.documentElement.lang = value;
  try {
    localStorage.setItem("league-replay:language", value);
  } catch {
    /* Still apply for this window. */
  }
  for (const notify of listeners) notify();
}
const subscribe = (notify: () => void) => {
  listeners.add(notify);
  return () => {
    listeners.delete(notify);
  };
};
const errorKeys: Record<string, Key> = {
  "game.invalidId": "identityError",
  "archive.unsupportedSchema": "storageError",
  "archive.invalidSchema": "storageError",
  "game.historyUnreadable": "historyUnreadable",
  "game.partialHistory": "historySyncPartial",
  "accounts.invalid": "savedAccountInvalid",
  "replay.destinationExists": "destinationExists",
  "replay.apiUnavailable": "apiUnavailable",
  "replay.seekWrongGame": "seekWrongGame",
  "replay.invalidTime": "seekWrongGame",
  "following.historyLimit": "historyLimit",
  "identity.invalidLink": "identityInvalid",
  "identity.overlap": "identityOverlap",
  "session.matchActive": "matchActive",
  "desktop.settingsError": "desktopSettingsError",
  "ui.conflict": "conflictTitle",
  "backup.invalid": "backupInvalid",
  "backup.version": "backupVersion",
  "backup.tooLarge": "backupTooLarge",
  "backup.changed": "backupChanged",
  "backup.offlineOnly": "backupOffline",
  "backup.busy": "backupBusy",
  "backup.writeFailed": "backupWriteFailed",
  "backup.restoreFailed": "backupRestoreFailed",
  "replay.notCancellable": "taskChanged",
  "replay.cancelled": "cancelled",
  "following.limit": "followLimitError",
  "following.busy": "followBusyError",
  "following.invalidLabel": "followLabelError",
  "following.changed": "sessionChanged",
  "session.invalidTitle": "sessionTitleError",
  "session.changed": "sessionChanged",
  "session.sameDestination": "sessionSame",
  "session.activePrefix": "sessionActivePrefix",
  "session.alreadyAttributed": "sessionAlready",
  "player.enterRiotId": "enterId",
  "player.notFound": "notFoundError",
  "player.riotClientUnavailable": "loginError",
  "player.ambiguous": "identityError",
  "gateway.network": "networkError",
  "client.Unavailable": "networkError",
  "client.NotFound": "clientDataPending",
  "client.Unauthorized": "tokenError",
  "client.InvalidData": "identityError",
  "gateway.unavailable": "networkError",
  "gateway.tokenNotReady": "tokenError",
  "gateway.unauthorized": "tokenError",
  "gateway.forbidden": "accessDeniedError",
  "gateway.rateLimited": "rateError",
  "gateway.notFound": "replayNotFound",
  "gateway.dataNotFound": "gatewayDataMissing",
  "region.switchClient": "regionError",
  "region.unsupported": "routeError",
  "region.operatorMismatch": "operatorError",
  "game.identityMismatch": "identityError",
  "game.invalidData": "identityError",
  "game.invalidTimeline": "timelineFormatError",
  "game.playerMissing": "identityError",
  "library.storageError": "storageError",
  "replay.storageError": "storageError",
  "replay.versionMismatch": "versionError",
  "replay.clientBusy": "busyError",
  "replay.clientFamilyMismatch": "regionError",
  "replay.clientNotReady": "tokenError",
  "replay.installMissing": "installError",
  "replay.installAmbiguous": "installAmbiguous",
  "replay.vanguardActive": "vanguardError",
  "replay.protectionUnavailable": "protectionError",
  "replay.openFailed": "openError",
  "replay.invalidFile": "fileError",
  "replay.incomplete": "fileError",
  "replay.tooLarge": "fileError",
  "replay.missing": "missing",
  "replay.clientPreparing": "preparing",
  "client.IdentityChanged": "identityError",
};
const errorMessages: Record<string, keyof typeof uiMessages> = {
  "ui.pendingSave": "标记尚未保存，请先完成保存再操作对局。",
  "ui.unreadable": "用户标记无法读取，原始数据已保留",
  "ui.tooLarge": "用户标记超过 2 MiB，请先导出备份。",
  "ui.invalidTheme": "主题无效",
  "ui.invalidNote": "笔记内容或时间无效",
  "ui.invalidPlayerName": "玩家名称无效",
  "identity.alreadyLinked": "一个账号只能关联一个玩家；请先解除原关联。",
  "session.clientActive": "当前正在排队或对局中，结束后再整理场次。",
  "session.ended": "该场次已结束，请刷新后重试。",
  "game.notFinished": "对局尚未结算",
  "game.missingStartTime": "缺少真实开局时间，无法补录",
};
export const translate = (
  key: Key,
  values: Record<string, string | number> = {},
) =>
  messages[key][index[locale]].replace(/\{(\w+)\}/g, (_, name: string) =>
    String(values[name] ?? ""),
  );
export function explainError(reason: unknown) {
  const code =
    reason instanceof IpcError
      ? reason.code
      : typeof reason === "string"
        ? reason
        : "";
  if (Object.hasOwn(errorMessages, code)) return ui(errorMessages[code]);
  return translate(
    Object.hasOwn(errorKeys, code)
      ? errorKeys[code]
      : code.startsWith("client.")
        ? "loginError"
        : "generalError",
  );
}
export function useI18n() {
  const language = useSyncExternalStore(subscribe, () => locale);
  const t = (key: Key, values: Record<string, string | number> = {}) =>
    messages[key][index[language]].replace(/\{(\w+)\}/g, (_, name: string) =>
      String(values[name] ?? ""),
    );
  return {
    language,
    t,
    date: (value: number | string) =>
      new Date(value).toLocaleString(language, {
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }),
    error: explainError,
  };
}
