# BELL Suite 共同契約 v1

- 版本：`suite-contract v1 (2026-11-08)`，修訂 d（見文末修訂紀錄）。依 Ada 2026-11-08 決定（優先於 `SPEC.md` 初稿的不同提議）。
- 可執行定義：`v1/reference.mjs`（無相依）。`node contracts/v1/check.mjs` 用 `v1/fixtures/` 驗證，必須 exit 0。
- 各產品在自己的 repo 保留型別與 zod 副本，檔頭寫 `suite-contract v1`；**不建立跨 repo 編譯相依**。
- 規則有疑義時，以 `reference.mjs` 與 fixtures 為準；改規則要同時改兩者並記入修訂紀錄；改變 payload 形狀才升 `schemaVersion`。

## 1. 設定：瀏覽器連結與伺服器讀取分開

| 變數 | 用途 | 允許 |
|---|---|---|
| `BELL_SUITE_LINKS`（Explain）、`BELLPERSONAL_SUITE_LINKS`（Personal） | 瀏覽器導覽用的姊妹 origin（切換器、深連結、交接、回程） | `appId=origin`，逗號分隔，最多 3 筆。loopback（`127.0.0.0/8`、`[::1]`、`localhost`）可 http／https；其他一律 **https**，且必須是 `*.ts.net` MagicDNS 名稱、與本 app 自己 `ALLOWED_ORIGINS` 的 tailnet 相同。 |
| `BELLPERSONAL_SUITE_FETCH`（僅 Personal） | Personal 伺服器讀姊妹的 `/api/suite/today` | `explain=` 或 `pr=`，只接受 `http://127.x.x.x:port` 或 `http://[::1]:port`（IP 字面值，不做 DNS，連 `localhost` 都不收）。 |

兩者都拒絕：帳密、路徑（含結尾 `/`）、query、fragment、萬用字元、port 0、重複 app、指向自己。設定錯誤在啟動時以 zh-TW 訊息拒絕開機，指名變數。

只使用驗證後的正規化 origin（`new URL(value).origin`），原字串不再出現在任何連結、比較或回程網址：`127.1` → `127.0.0.1`、`[0:0:0:0:0:0:0:1]` → `[::1]`、host 轉小寫、預設埠（http 80、https 443）省略（D4／D5，fixtures `config.cases.json`）。`localhost` 仍可設定，但它與 `127.0.0.1` 是不同 site（cookie、storage 分開），本輪不以放寬 CSP、CORS、SameSite、Host 或 auth 讓兩者互通。**本輪支援的示範是同一個 `127.0.0.1`、不同 port**（例：Explain `127.0.0.1:21101`、Personal `127.0.0.1:21103`；同 site、不同 origin）；正式環境是同一 tailnet 的 `*.ts.net` https。

不擴大 CSP、CORS、Host、Origin 或 auth：姊妹 origin 只出現在 `<a href>` 與 `location` 導覽，不進 `connect-src`，不加 CORS 標頭。請求參數不能指定或改變讀取目標。

## 2. `GET /api/suite/manifest`

同源讀取，在既有守門（Host／Origin／Sec-Fetch-Site；Personal 另有 tailscale 身分閘）之後，`Cache-Control: no-store`。

```ts
type SuiteAppId = "explain" | "personal" | "pr";
interface SuiteManifest {
  schemaVersion: 1;
  appId: SuiteAppId;
  displayName: string;            // ≤ 40
  summary: string;                // ≤ 40
  capabilities: Array<{
    id: string;                   // 例 "explain.draft"
    kind: "accept-handoff" | "today-feed" | "object-route";
    route?: string;               // "#/…"，可含 {id}
    api?: string;                 // "/api/…"，同源路徑
    accepts?: ["text"];           // 只限 accept-handoff
    maxChars?: number;            // 只限 accept-handoff，≤ 2000
  }>;                             // ≤ 16
  links: Partial<Record<SuiteAppId, string>>; // 只放已驗證的瀏覽器 origin；未設定就不出現
}
```

前端只讀**自己**的 manifest；`links` 沒有的 app 顯示「未連結」，不假裝在線。

## 3. `GET /api/suite/today`（Explain 提供）

```ts
interface SuiteObjectRef { sourceApp: SuiteAppId; type: string; id: string; title: string; href: string }
interface SuiteTodayItem extends SuiteObjectRef {
  state: "due" | "new" | "waiting" | "needs-you";
  updatedAt: string;              // ISO UTC，Z 結尾
  dueAt?: string;
  detail?: string;                // ≤ 80
}
interface SuiteTodayFeed {
  schemaVersion: 1; appId: SuiteAppId; generatedAt: string;
  counts: Record<string, number>; // ≤ 8 個，非負整數
  items: SuiteTodayItem[];        // ≤ 5
  truncated: boolean;
}
```

- `id` 為 `[A-Za-z0-9._:-]{1,128}`；`title` ≤ 120、純文字、不含控制字元或 bidi 控制字元。
- `href` 是姊妹自己 origin 內的相對 hash route：`^#/[A-Za-z0-9/_\-.~%:]*(\?[A-Za-z0-9_\-.~%=&+]*)?$`，且不含 `//`，解碼後不含控制字元、反斜線，不以 `#//` 開頭；**並且必須完全符合來源 app 的允許路由**（§5.1 路由表）。
- **資料最小化**：只傳課程標題、計數與物件參照；不傳答案、提示、重點、筆記內文、文件內容。
- 嚴格驗證：未知欄位、任何一個無效項目 → 整份 feed 視為無效（`unavailable`），不部分採用。

## 4. `GET /api/suite/sources`（Personal 提供）

```ts
interface SuiteSourcesResponse {
  schemaVersion: 1;
  sources: Array<{
    appId: "explain" | "pr";
    status: "ready" | "stale" | "unavailable" | "unconfigured" | "unsupported";
    lastSuccessAt?: string;
    feed?: SuiteTodayFeed;        // 只有 ready／stale 帶
  }>;
}
```

| 狀態 | 意義 | `feed` | `lastSuccessAt` |
|---|---|---|---|
| `ready` | 60 秒內成功 | 有 | 有 |
| `stale` | 讀取失敗，但 30 分鐘內有成功 | 有（舊的） | 有 |
| `unavailable` | 讀取失敗且無 30 分鐘內成功，或版本不符 | 無 | 可有 |
| `unconfigured` | `BELLPERSONAL_SUITE_FETCH` 沒設定這個 app | 無 | 無 |
| `unsupported` | 該 app 尚未提供 today-feed（目前的 PR） | 無 | 無 |

讀取限制（每個來源）：3 秒總逾時、回應串流 64 KB 上限（超過即中止）、不跟隨 redirect、只收 `application/json` 的 200。快取只在記憶體：進頁時超過 60 秒才重讀、頁面在前景時每 5 分鐘更新、`?refresh=1` 手動重試（與進行中的讀取合併），超過 30 分鐘沒有成功就丟掉舊 feed。**未知不能顯示 0**：沒有 feed 時 UI 顯示「—」與狀態，不顯示數字。

單向：Personal 只讀姊妹；姊妹不能寫 Personal，本契約沒有任何寫入端點。

## 5. 交接（fragment）

| 目標 | 格式 |
|---|---|
| Explain | `<explain-origin>/#/explain?text=…&title=…&from=personal&back=…` |
| Personal | `<personal-origin>/#/tasks?draft=…&from=explain&back=…` |

- 用 `URLSearchParams` 編碼與解析（`+` 代表空白）。`text`／`draft` ≤ 2000 字元（超過截斷並標示），`title` ≤ 120（只有 Explain 收）。
- `text`、`title`、`draft`、`from`、`back` 任一個出現兩次以上（以 `URLSearchParams` 解碼後的鍵計，`dr%61ft` 也算 `draft`；兩份值相同也算），整個交接拒絕（`{ rejected: "duplicate" }`），並照下方規則先刪舊草稿；不取第一份也不取最後一份，避免不同解析器讀到不同內容（D9）。其他無關的鍵重複不影響。重複的判斷在「缺少必要鍵」之前：`#/tasks?from=explain&from=pr` 也是 `duplicate`。
- 交接嘗試（修訂 d）：只在目標的交接入口判斷，路徑必須完全相同（Explain `#/explain`、Personal `#/tasks`）。`text`、`title`、`draft`、`from`、`back` 任一個出現（空值、`#/tasks?back` 這種沒有 `=` 的鍵、百分比編碼的鍵名都算；鍵名分大小寫），就是一次新的交接嘗試：先把內容從網址移除並刪除舊草稿，再驗證。`parseHandoff` 回傳 `undefined` 以外的值，就等於這種情形（`isHandoffAttempt`）。
- 缺少這個目標的必要鍵（Explain 的 `text`、Personal 的 `draft`），例如只有 `from`／`back`／`title`，或把另一邊的鍵送錯地方 → 拒絕（`{ rejected: "missing" }`）。所有拒絕（`duplicate`、`missing`、`from`、`empty`）都顯示可復原的錯誤：說明已忽略這個連結、可以關閉，頁面其他功能照常；不呼叫 AI，也不寫產品資料。
- 完全沒有保留鍵的入口（`#/tasks`、`#/tasks?group=someday`、`#/explain`）與其他路由（例如 Explain 的 `#/note/{id}/{section}?from=personal`、Personal 的 `#/tasks/{id}`）都不是交接，沿用原本的新增／恢復流程，不刪草稿。
- `from` 必須是接收方已設定 `links` 的 app、且不是自己；否則拒絕（`{ rejected: "from" }`）。
- `back` 必須通過與 `href` 相同的規則、≤ 512，並**完全符合 `from` app 的允許路由**（§5.1）；不通過就丟掉回程、保留草稿。回程網址由接收方組成：`links[from] + "/" + back`，絕不使用交接帶來的 origin。
- 文字清理：CRLF 正規化、移除控制字元與 bidi 控制字元（保留 `\t`、`\n`）、trim；清理後為空 → 拒絕（`{ rejected: "empty" }`）。
- 接收後立刻 `history.replaceState` 把內容從網址移除；草稿存在 `sessionStorage`（例 `bell.suite.handoff.tasks`，`{v:1,text,from,backUrl?,receivedAt}`），新交接完整取代舊的，不混用舊草稿與新 `from`。
- 純文字顯示；開頁或預填**不會**呼叫 AI、不寫產品資料。只有使用者明確按「生成／建立」才執行，且按鈕防重複提交。
- 方向與來源（D7）：草稿正文與標題保留使用者文字，包括 U+200E／U+200F（只移除上面的控制與 bidi 覆寫／隔離字元），接收方不另改寫。顯示時以 `<bdi>` 或 `dir="auto"` 的輸入框隔離方向，不讓使用者文字改變周圍介面文字的方向。來源名稱一律由接收方依 `from` 的 app id 從自己的固定表取得（例：`explain` → 「BELL Explain」），交接不帶、也不採用任何自稱的來源名稱。
- 風險說明：fragment 不會送到伺服器，也不進 Referer；但在接收方清除前，內容會短暫出現在網址列，可能被瀏覽器擴充功能、同步的瀏覽紀錄或螢幕看到。不宣稱零風險，敏感內容請使用者自行判斷。

### 5.1 允許路由（`href` 與 `back` 共用；`reference.mjs` `ROUTES`／`allowedRoute`）

`{id}` 是單一段 `[A-Za-z0-9._:-]{1,128}`，不是 `.` 或 `..`；不含 `%`，所以雙重編碼、`?`、`/` 都進不來。送出方要放**原始 id**（不要 `encodeURIComponent`，`:` 原樣保留）；id 不符合時改送清單路由（例 `#/events`）。

| app | 允許的路由 |
|---|---|
| `explain` | `#/`、`#/library`、`#/graph`、`#/favorites`、`#/repositories`、`#/review`、`#/note/{id}` |
| `personal` | `#/today`、`#/tasks`、`#/tasks/{id}`、`#/events`、`#/events/{id}`、`#/ideas`、`#/ideas/{id}`、`#/inbox`、`#/inbox/{id}` |
| `pr` | 本輪無（adapter 屆時加入 `#/runs/{id}/{tab}`） |

## 6. 「解釋這個」

不限郵件：任務、靈感、活動、郵件等文字上下文都可送到 Explain，附回程。desktop 以側欄、mobile 以全寬 dialog／抽屜呈現來源詳情，360px 不水平捲動。zh-TW 優先、亮／暗、鍵盤與焦點可見，loading／empty／error／stale 都有畫面。

## 7. 設計 token v1

各 repo 保留小副本並帶 `suite-tokens v1` 註記：`--bell-surface`、`--bell-surface-raised`、`--bell-text`、`--bell-text-muted`、`--bell-border`、`--bell-accent`、`--bell-focus`、`--bell-radius`、`--bell-shadow`。值由各 app 對應到自己的主題變數；視覺稿在 `bell-suite/design/`（Pixel）。

## 8. PR adapter（本輪只有規格）

BELL PR 目前沒有 follow-ups API。本輪：Personal 對 `pr` 只回 `unconfigured`（沒設定）或 `unsupported`（有設定連結或讀取，但 PR 尚無 today-feed），不假造數字。日後 PR 若提供 `GET /api/suite/today`，只需符合 §3，`counts` 建議 `{ needsYou, waiting }`、項目 `state: "needs-you" | "waiting"`、`href` 指向 PR 自己的 hash route；Personal 端改為 `ready` 即可，不改契約版本。

## 9. 修訂紀錄與待定項

- **修訂 b（2026-11-08）**：依 Ada「back 為該 app 允許的相對 hash route」與 Rigel SPEC v0.2 §8 的 D1–D3、D8 收緊：`href`／`back` 必須完全符合來源 app 的允許路由（§5.1），因此 `%`、`?`、`..` 與不在表上的路由一律丟掉；路由路徑字元加入 `:`，讓 `{id}` 不需編碼；Personal 路由表加入本輪新增的 `#/tasks/{id}`、`#/events`、`#/events/{id}`、`#/ideas/{id}`；明訂帶交接參數就先刪舊草稿。payload 形狀不變，`schemaVersion` 仍為 1。
- **修訂 c（2026-11-08）**：Ada 裁定 D4／D5／D7／D9。D4／D5：`localhost`、非正規 loopback 與預設埠照舊接受，但只用正規化後的 origin（§1），本輪示範為同一 `127.0.0.1` 不同 port，`localhost` ↔ `127.0.0.1` 不以放寬守門解決；D7：保留使用者文字（含 U+200E／U+200F），以 `<bdi>`／`dir="auto"` 顯示，來源名稱由接收方固定表取得（§5）；D9：`text`／`title`／`draft`／`from`／`back` 重複即拒絕整個交接並刪舊草稿，取代原本的「取第一個」（§5）。新增 fixtures：`handoff.cases.json` 的 D7／D9 案例、`config.cases.json` 的正規化案例。payload 形狀不變，`schemaVersion` 仍為 1。
- **修訂 d（2026-11-08）**：Ada 裁定交接嘗試（回應 Rigel 指出 rev. c「沒有 `text`／`draft` 不是交接」與「帶任何交接參數就先刪舊草稿」互相矛盾）。目標交接入口出現任一保留鍵（`text`／`title`／`draft`／`from`／`back`）就是交接嘗試，先清網址與舊草稿再驗證；缺少該目標的必要鍵（例如只有 `from`／`back`／`title`）回傳新的 `{ rejected: "missing" }`，清理後為空（`empty`）也改為顯示錯誤，而不是靜默忽略；沒有保留鍵的入口與其他路由不受影響。新增 `isHandoffAttempt(hash, target)`；`handoff.cases.json` 原「沒有 draft 的重複鍵不是交接」改為 `duplicate`，並新增 21 個嘗試／普通入口案例（名稱前綴 E1–E11 對應 `verification/QA-PLAN.md` 交接入口表的同一網址片段），`check.mjs` 對每個案例核對 `isHandoffAttempt` 與預期一致。payload 形狀不變，`schemaVersion` 仍為 1。
