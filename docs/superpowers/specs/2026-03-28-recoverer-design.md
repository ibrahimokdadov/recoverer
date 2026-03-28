# Recoverer — Design Specification
**Date:** 2026-03-28
**Status:** Approved

---

## Overview

Recoverer is a Windows desktop GUI application that scans drives or folders for deleted files and recovers them. It identifies file types by reading raw file signatures (magic bytes), supports filtering by type, and guides users from scan to recovery in a calm, modern interface.

**Primary user:** A non-technical person in a panic — they just deleted something important (vacation photos, work document) and need it back without reading a manual.

---

## Product Decisions

### Business Model
- **Freemium, one-time unlock.** No subscription.
- **Free tier:** Full scan, full preview, recover up to 5 files per session (no file size cap).
- **Paid tier:** $29.99 one-time — unlimited recovery, batch recovery.
- Free tier must prove the tool works before users pay.

### App Name
- Default: "Recoverer" — own the literal clarity ("does one thing: gets your files back").
- Alternatives if rebranding: **Rewind** (most memorable), **Undelete** (best SEO), **Salvage** (strongest connotation).

### File Type Priority (v1)
1. Images: JPEG, PNG, HEIC, GIF, BMP, WebP
2. Videos: MP4, MOV, AVI, MKV, WMV
3. Documents: DOCX, PDF, TXT, RTF, ODT
4. Spreadsheets: XLSX, CSV, ODS
5. Archives: ZIP, RAR, 7Z, TAR
6. Audio: MP3, FLAC, WAV, AAC, OGG
7. RAW photos: CR2, NEF, ARW, DNG
8. Presentations: PPTX, ODP
9. Email archives: PST, MBOX
10. Creative: PSD, AI, INDD

---

## Technical Architecture

### Technology Stack

| Layer | Technology | Rationale |
|---|---|---|
| Core engine | Rust | Memory safety for parsing untrusted raw disk data, no GC pauses during I/O-intensive scans, `windows-rs` for Win32 APIs |
| UI shell | C# / WinUI 3 | Native Windows 11 look, rich databinding, virtualized lists, fast iteration |
| IPC | Named pipe (newline-delimited JSON) | Crash isolation — corrupt disk data crashing the engine won't kill the UI |
| Result storage | SQLite (`rusqlite`) | Handles 100k+ file metadata rows trivially, survives pause/resume |
| File type detection | `infer` crate + custom signature table (seeded from PhotoRec's ~500 signature list) | No external dependencies, thread-safe, extensible |
| Async orchestration | Tokio | Non-blocking scan orchestration |
| CPU-bound carving | Rayon thread pool | N workers = CPU core count |

### Recovery Techniques (always run all three, in order)

1. **VSS snapshot enumeration** — check Windows Volume Shadow Copies first. Instant perfect recovery for recently deleted files within the retention window. Lowest complexity, highest confidence. Run before raw disk operations.

2. **NTFS MFT scan** — read the Master File Table for deleted file entries (flag `FILE_RECORD_IN_USE` cleared). Extracts original filename, timestamps, file size, and exact cluster locations (data run list). High confidence, fast. Uses libtsk via FFI.

3. **Raw cluster carving** — scan unallocated clusters (using `$Bitmap` to skip allocated space) for known file magic bytes. Catches files whose MFT entries have been reused. Slower, but thorough. Custom carving engine with PhotoRec signature database.

**FAT32/exFAT volumes:** Parse FAT directory entries (deleted entries marked `0xE5`) + raw carving.

### File Type Detection

Never trust file extensions. Layered approach:

1. `infer` crate: fast first-pass magic byte detection (~60+ types)
2. Custom signature table: extended coverage from PhotoRec's database
3. For Office Open XML (DOCX/XLSX/PPTX): detect ZIP magic, then inspect `[Content_Types].xml` inside to disambiguate
4. Format: return MIME type string (`image/jpeg`) + category (`Images`)

### Component Architecture

```
UI Shell (C# / WinUI 3)
    │  Named Pipe (JSON events)
    ▼
Recoverer Engine (Rust)
    ├── Scan Orchestrator (Tokio async)
    │       ├── VSS Enumerator
    │       ├── MFT Scanner (libtsk FFI)
    │       └── Carving Engine (Rayon workers)
    ├── File Type Detector (infer + custom)
    ├── Result Store (SQLite)
    └── Recovery Writer (verified copy, different-volume check)
```

### Engine IPC Events (newline-delimited JSON over named pipe)
```json
{ "event": "progress", "phase": "mft_scan", "pct": 23, "files_found": 1247, "eta_secs": 840 }
{ "event": "file_found", "id": 1248, "filename": "vacation_01.jpg", "size": 5242880, "mime": "image/jpeg", "category": "Images", "confidence": 62, "original_path": "C:\\Users\\...\\Pictures" }
{ "event": "phase_change", "new_phase": "carving" }
{ "event": "complete", "total_found": 4218, "duration_secs": 1247 }
{ "event": "error", "code": "VOLUME_ACCESS_DENIED", "message": "...", "fatal": false }
```

### SQLite Schema
```sql
CREATE TABLE files (
    id INTEGER PRIMARY KEY,
    filename TEXT,                 -- NULL if only carved (no MFT entry)
    original_path TEXT,            -- NULL if path not recoverable
    mime_type TEXT NOT NULL,       -- e.g., "image/jpeg"
    category TEXT NOT NULL,        -- e.g., "Images"
    size_bytes INTEGER,
    first_cluster INTEGER,
    confidence INTEGER,            -- 0–100
    source TEXT NOT NULL,          -- "vss", "mft", "carved"
    recovery_status TEXT DEFAULT 'pending',  -- pending/recovered/failed/skipped
    mft_record_number INTEGER,
    created_at INTEGER,            -- Unix timestamp
    modified_at INTEGER,
    deleted_at INTEGER
);
CREATE INDEX idx_category ON files(category);
CREATE INDEX idx_confidence ON files(confidence);
CREATE INDEX idx_filename ON files(filename);
```

### Admin Elevation
- UAC manifest on both executables: `<requestedExecutionLevel level="requireAdministrator" />`
- App prompts for elevation at launch — no silent re-launch tricks
- Detect if volume handle fails to open → show clear inline error with "Restart as Administrator" button
- Warn proactively if SSD TRIM is detected (IOCTL_STORAGE_QUERY_PROPERTY) — explain recovery may be limited

### Packaging
- **WiX v4 MSI** (primary installer) — handles admin elevation, upgrades, enterprise deployment
- **Portable ZIP** — for users running on a borrowed machine
- **EV code signing certificate** — mandatory before public release (SmartScreen will block unsigned admin-requesting executables)

### Critical Risks

| Risk | Impact | Mitigation |
|---|---|---|
| SSD TRIM erases deleted data | HIGH | Detect TRIM support, warn user prominently before/during scan |
| Cluster reallocation after deletion | HIGH | Warn user "stop using this drive NOW" in the UI — time is critical |
| Corrupt MFT entries crash parser | HIGH | Wrap every MFT record parse in error handling; log and skip corrupt entries |
| Writing recovered files to source volume overwrites data | CATASTROPHIC | Detect source/destination drive match; refuse with clear error |
| Anti-virus interference | MEDIUM | EV code signing, document AV exclusion steps |
| Fragmented files produce truncated carve results | MEDIUM | MFT data run list handles fragmentation; document limitation for MFT-less recoveries |
| Very large volumes take hours | MEDIUM | Support pause/resume with SQLite checkpoint; show real ETA |

---

## UX Design

### Layout Pattern
**Hybrid Wizard + Persistent Left Navigation Rail**

4 stages in the left rail, locked/unlocked progressively:
1. **Setup** — always accessible
2. **Scanning** — unlocks when scan starts
3. **Results** — unlocks when scan completes or is stopped
4. **Recovery** — unlocks when files are selected

Left rail: 200px expanded / 48px icon-only collapsed. No sidebars until Results screen.

---

### Screen 1: Scan Setup

**Controls:**
- **WHERE TO SCAN:**
  - Radio: "Entire Drive" → dropdown (drive letter, friendly name, used/total GB)
  - Radio: "Specific Folder" → text field + Browse button
- **WHAT TO LOOK FOR (optional):**
  - Checkboxes: Images ✓, Videos ✓, Documents ✓, Audio ✓, Archives ✓, Other ✓ (all default on)
  - Text field: "File name contains" (optional)
- **SCAN DEPTH:**
  - Radio: Quick Scan (~2–5 min) — finds recently deleted files
  - Radio: **Deep Scan** (~15–45 min) — **default**, recommended
- **CTA:** Single "Start Scan →" button (bottom right, primary/accent color)

**First-launch only:** Dismissable banner: *"The sooner you recover, the better. Avoid saving new files to the same drive until recovery is complete."*

---

### Screen 2: Scan Progress

**Elements:**
- Progress bar: real percentage + estimated time remaining
- Phase label: "Checking Volume Shadow Copies..." / "Scanning file records..." / "Deep scanning unallocated space..."
- **Live discovery feed:** scrolling list of newly found files (newest first), fades in with 150ms ease-out. This is the emotional core — seeing "vacation_01.jpg" appear reassures the user.
- Category breakdown: 5 rows (Images/Videos/Documents/Audio/Archives) each with count + inline relative bar that grows live
- Currently scanning path: ticker (truncated with ellipsis)
- Buttons: **[Pause]** and **[Cancel]**

**Cancel behavior:** Dialog — "Stop scanning? You can still recover the X files found so far, but may miss additional files." → [Continue Scanning] / [Stop and View Results]

**Pause state:** Progress area replaced with "Paused" label + [Resume] button. Engine checkpoints current cluster position to SQLite.

---

### Screen 3: Results

**Three zones:**
- **Left sidebar (200px):** Filter panel
- **Center:** Results list/grid with action bar
- **Bottom drawer:** Preview pane (appears when exactly 1 file selected)

**Filter sidebar:**
- File type checkboxes (same as Setup but now with live counts)
- Date deleted: Any / Today / This week / This month / Custom range
- Recovery confidence: All / High only (≥80%) / Good (≥50%)
- Quick action: "Select All High Confidence" button

**Results list columns (default List view):**
- Checkbox | Type icon | File Name | Size | Date | Confidence | Original Path (truncated)

**Confidence indicator:** 5-dot scale ●●●●● color coded:
- ●●●●● 95–100%: green — "Excellent — full recovery expected"
- ●●●●○ 75–94%: teal — "Good — minor data loss possible"
- ●●●○○ 50–74%: amber — "Fair — file may be partially corrupt"
- ●●○○○ 25–49%: orange — "Poor — significant data loss likely"
- ●○○○○ 0–24%: red — "Low — file mostly overwritten"
- Plain English shown in tooltip on hover

**View modes (toggle):** List (default) | Grid (thumbnails, for images) | Group (by category)

**Search bar:** Live filter (200ms debounce), searches filename. Dropdown: "Also search original path."

**Selection:** Checkboxes, Shift+click range, Ctrl+A selects all visible. "Select All Visible" header checkbox respects active filters.

**Bottom action bar:** "[X files selected]  [Recover Selected →]" — sticky, always visible.

**Preview pane (single selection):**
- Images: rendered thumbnail from recovered data, confidence badge
- Documents: first ~20 lines of text, monospace
- Video/audio: metadata only (filename, size, duration if parseable)
- Binary/unknown: hex preview, first 256 bytes
- Low confidence: amber warning "This file was partially overwritten. Recovery may produce a corrupt or incomplete file."

---

### Screen 4: Recovery

**Destination modal (on "Recover Selected →"):**
- Destination folder picker — auto-suggests a different drive (never defaults to source)
- Inline warning if user picks source drive (not a hard block — confirmed dialog)
- Options: "Recreate original folder structure" (default) or "Save all files flat"
- Naming conflicts: "Add number suffix" (default) / Skip duplicates / Overwrite
- CTA: [Start Recovery →]

**Recovery progress (modal):**
- Progress bar: X of N files
- Current file being written
- Live tally: ✓ Recovered: N | ⚠ Recovered with warnings: N | ✕ Failed: N
- [Cancel Remaining] button

**Recovery complete:**
- Summary: ✓ N recovered | ⚠ N with warnings | ✕ N failed
- Details of warning/failed files with plain English explanations
- Buttons: [View in File Explorer] | [View Recovery Report] | [Scan Again]
- "View Recovery Report" opens a plain-text log saved alongside recovered files

---

### Error States

**No files found:** Not just "No results" — explain why (SSD TRIM? Files deleted long ago? Type filters too narrow?) and offer 2–3 concrete next steps as buttons.

**Permission errors during scan:** Inline amber banner above results — not a blocking modal. Lists inaccessible paths. Offers "Restart as Administrator." Dismissable.

**Low confidence files:** Never blocked from recovery. Warning shown in confidence column, preview pane, and recovery destination modal ("3 files in your selection have low recovery confidence").

**Recovery failure (entire recovery):** Clear message with count of already-recovered files (never deleted) + "Choose New Destination" path forward.

**Same-drive recovery warning:** Confirmation dialog (not hard block) — "Saving to the same drive could overwrite other recoverable files. Recommended: choose a different drive." → [Choose Different Drive] / [I understand, continue anyway]

---

### Visual Style

**Theme:** Dark mode primary. Full light mode via Windows system theme. Respect `Windows.UI.ViewManagement.UISettings.GetColorValue(UIColorType.Background)`.

**Color palette:**
```
Dark mode:
  Base bg:       #0F0F0F
  Surface:       #1A1A1A
  Elevated:      #242424
  Border:        #333333
  Text primary:  #F0F0F0
  Text secondary:#A0A0A0

Accent:          #3A9FDB  (trustworthy blue — NOT green/red)
Success:         #3DB87A
Warning:         #E0A020
Error:           #D95858
```

**Typography:** Segoe UI Variable (Windows 11 system font). Page titles: 20px/600. Section headers: 13px/600/uppercase. Body: 14px/400. Captions: 12px/400.

**Icons:** Segoe Fluent Icons (system) for navigation/controls. Colored file-type icons for categories in results.

**Motion:** Minimal. Discovery feed items: fade+slide in 150ms ease-out. Progress bar: smooth continuous fill. No spring animations, no confetti.

**Geometry:** 8px spacing grid. Corner radius: 6px cards/panels, 4px buttons/inputs. No heavy drop shadows — use background + border for layer differentiation.

---

## Success Metrics (v1)

- Recovery success rate ≥ 80% on files deleted within 7 days on an undisturbed HDD
- Scan completion without crash: 99%+ on drives up to 2TB
- User can complete a recovery of a known file within 5 minutes of first launch (no tutorial)
- App Store / review rating ≥ 4.2 stars within 90 days
- Free-to-paid conversion ≥ 8%
- Zero support tickets of the form "I couldn't figure out how to start a scan"
