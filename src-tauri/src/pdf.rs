//! File › Print › Save as PDF: the pages shown for printing, written straight to a PDF by the
//! WebView (WebView2 on Windows) at the paper size and margins, with no browser header or footer. Elsewhere the window's print dialog is used instead.
use std::path::Path;
use tauri::WebviewWindow;

#[cfg(windows)]
pub async fn write(w: &WebviewWindow, path: &Path, w_mm: f64, h_mm: f64, margins: [f64; 4]) -> Result<(), String> {
    use webview2_com::{Microsoft::Web::WebView2::Win32::*, PrintToPdfCompletedHandler};
    use windows_core::{Interface, HSTRING};
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), String>>();
    let file = HSTRING::from(path.as_os_str());
    let (short, long, landscape) = (w_mm.min(h_mm) / 25.4, w_mm.max(h_mm) / 25.4, w_mm > h_mm);
    let done = tx.clone();
    w.with_webview(move |wv| unsafe {
        let run = || -> windows_core::Result<()> {
            let core = wv.controller().CoreWebView2()?;
            let s = wv.environment().cast::<ICoreWebView2Environment6>()?.CreatePrintSettings()?;
            s.SetOrientation(if landscape { COREWEBVIEW2_PRINT_ORIENTATION_LANDSCAPE } else { COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT })?;
            s.SetPageWidth(short)?;
            s.SetPageHeight(long)?;
            let [t, r, b, l] = margins.map(|m| m / 25.4);
            s.SetMarginTop(t)?;
            s.SetMarginRight(r)?;
            s.SetMarginBottom(b)?;
            s.SetMarginLeft(l)?;
            s.SetScaleFactor(1.0)?;
            s.SetShouldPrintBackgrounds(true)?;
            s.SetShouldPrintHeaderAndFooter(false)?;
            let tx = done.clone();
            let handler = PrintToPdfCompletedHandler::create(Box::new(move |hr, ok| {
                let _ = tx.send(hr.map_err(|e| e.message()).and_then(|_| if ok { Ok(()) } else { Err("the PDF could not be written".into()) }));
                Ok(())
            }));
            core.cast::<ICoreWebView2_7>()?.PrintToPdf(&file, &s, &handler)
        };
        if let Err(e) = run() { let _ = done.send(Err(e.message())); }
    }).map_err(|e| e.to_string())?;
    drop(tx);
    tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(180)).map_err(|_| "Writing the PDF took too long.".to_string())?)
        .await.map_err(|e| e.to_string())?
}

#[cfg(not(windows))]
pub async fn write(_: &WebviewWindow, _: &Path, _: f64, _: f64, _: [f64; 4]) -> Result<(), String> {
    Err("native-pdf-unavailable".into())
}
