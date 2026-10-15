# Print findings (Milestone 1 risk check)

**Status: NOT YET VERIFIED.** The 4x6 test sheet (`ui/src/lib/PrintTest.svelte`, "Print test" button
in the app header) is implemented, but printing needs a human at a real window. Fill in one section per platform.

## How to test
1. `npm run tauri dev`, click **Print test**, click **Print 4x6 test sheet**.
2. Print to "Microsoft Print to PDF" (Windows) / "Save as PDF" (macOS), then to a real printer if available, at 100% / actual size.
3. Measure the blue grid squares (should be 1 in), the red frame inset (0.25 in), page size (4x6 in) and page count (1).

## Windows (WebView2)
- Date / OS / WebView2 version:
- `window.print()` opened the system dialog:
- Measured grid square:
- Page size / page count:
- Margin behaviour:
- Verdict (true-size printing: works / needs workaround):

## macOS (WKWebView)
- Date / OS version:
- `window.print()` opened the system dialog:
- Measured grid square:
- Page size / page count:
- Margin behaviour:
- Verdict (true-size printing: works / needs workaround):

## Workaround to try in Milestone 5 if either fails
Generate a PDF in Rust at the exact page size and open it with the system viewer for printing.
