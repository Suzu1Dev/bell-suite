# BELL Suite（BELL 產品套件）

[English Documentation (README.md)](./README.md)

**BELL Suite** 是一套以本機優先（Local-First）、重視隱私保護為核心的五合一工作空間。五款工具皆可**零環境變數獨立運作（Standalone Mode）**，也能透過 `bell-suite` 串聯成完整的五合一產品套件（支援 `⌃1`–`⌃5` 快速切換、Personal 今日摘要彙整，以及零自動寫入的跨產品草稿交接）。

| 快捷鍵 | 產品名稱 | 獨立 CLI 指令 | Suite 預設埠號 | 核心定位 |
| :--- | :--- | :--- | :--- | :--- |
| `⌃1` | **BELL Personal** | `bell-personal` | `4601` | 今日總覽、待辦事項、行事曆、靈感與收件匣 |
| `⌃2` | **BELL Explain** | `bell-explain` | `4602` | 互動式概念拆解、程式碼逐步推演與 FSRS 間隔複習 |
| `⌃3` | **BELL PR** | `bell-pr` | `4603` | AI 輔助開源貢獻、程式碼理解與審查工作台 |
| `⌃4` | **BELL Web Reader** | `bell-reader` | `4604` | 純淨網頁擷取、紙本感螢光筆標註與手繪塗鴉筆記 |
| `⌃5` | **BELL Campaign** | `bell-campaign` | `4605` | 活動專案倒數時程、各組待辦、招工表單與行前手冊生成 |

---

## 快速安裝

### 1. 一鍵安裝（macOS / Linux / WSL2）

```sh
curl -fsSL https://raw.githubusercontent.com/Suzu1Dev/bell-suite/main/install.sh | sh
```

### 2. Windows PowerShell 安裝

```powershell
irm https://raw.githubusercontent.com/Suzu1Dev/bell-suite/main/install.ps1 | iex
```

### 3. 使用 `npx` 或從 GitHub 原始碼啟動

```sh
# 使用 npx（套件發布後可直接執行）
npx bell-suite doctor

# 或從 GitHub 複製原始碼
git clone https://github.com/Suzu1Dev/bell-suite.git
cd bell-suite
./bin/bell-suite.mjs setup   # 自動從 GitHub 下載並建置缺少的 BELL 應用程式至 ~/.local
./bin/bell-suite.mjs start   # 一鍵啟動五款工作空間並自動設定跨產品連結
```

---

## CLI 指令總覽（`bell-suite`）

```sh
bell-suite start                       # 於 4601–4605 啟動所有已安裝的 BELL 工作空間
bell-suite start --base-port 5600      # 自訂起始連接埠（5601–5605）
bell-suite start --only explain,reader # 僅啟動指定的子產品
bell-suite start --no-open             # 不自動開啟瀏覽器（適合遠端／伺服器環境）
bell-suite status                      # 探測各埠號並驗證 /api/suite/manifest 契約
bell-suite doctor [--json]             # 檢查 Node、Git、AI CLI 與各工具安裝狀態
bell-suite setup                       # 自動複製／更新並建置五款 BELL 應用程式
```

---

## 架構設計與 Suite Contract v1

BELL Suite 的每一款工具都擁有獨立程序與獨立的 SQLite 資料庫，**絕不共用資料庫，也不會直接跨服務寫入資料**：

1. **產品清單（`/api/suite/manifest`）**：宣告 `appId`、顯示名稱、支援能力與經過嚴格白名單驗證的本機／同 Tailnet 連結，驅動 `⌃1`–`⌃5` 切換器。
2. **今日摘要（`/api/suite/today`）**：嚴格資料最小化（僅輸出計數、標題與 `#/` 物件路由，絕不含內文或金鑰），供 BELL Personal 於本機迴路（`127.0.0.1`）唯讀彙整。
3. **雙向草稿交接（`#/<route>?draft=...&from=<app>&back=<hash>`）**：純瀏覽器端網址導覽，在目標工具開啟草稿預覽，使用者按下確認前絕不寫入資料庫，並保留安全的「回到來源」連結。

執行契約驗證與 CLI 測試：

```sh
npm test
```

## 授權條款

[Apache-2.0](./LICENSE) · [Trademarks](./TRADEMARKS.md)