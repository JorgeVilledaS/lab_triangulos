//! Perfilador por instrumentación, activado únicamente con --features profile.
//! Tiempos inclusivos y exclusivos; no es un perfil estadístico de CPU.
#[cfg(feature = "profile")]
mod enabled {
    use std::{cell::RefCell, collections::BTreeMap, time::Instant};
    #[derive(Default)]
    struct Entry {
        calls: u64,
        inclusive: u128,
        exclusive: u128,
    }
    #[derive(Default)]
    struct State {
        entries: BTreeMap<&'static str, Entry>,
        children: Vec<u128>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    pub struct Guard {
        name: &'static str,
        start: Instant,
    }
    pub fn scope(name: &'static str) -> Guard {
        STATE.with(|state| state.borrow_mut().children.push(0));
        Guard {
            name,
            start: Instant::now(),
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let elapsed = self.start.elapsed().as_nanos();
            STATE.with(|state| {
                let mut s = state.borrow_mut();
                let children = s.children.pop().unwrap_or(0);
                if let Some(parent) = s.children.last_mut() {
                    *parent += elapsed;
                }
                let entry = s.entries.entry(self.name).or_default();
                entry.calls += 1;
                entry.inclusive += elapsed;
                entry.exclusive += elapsed.saturating_sub(children);
            });
        }
    }
    pub fn save(directory: &std::path::Path) -> std::io::Result<()> {
        STATE.with(|state| {
            let s = state.borrow();
            let mut csv = String::from("scope,calls,inclusive_ms,exclusive_ms\n");
            let max = s.entries.values().map(|e| e.exclusive).max().unwrap_or(1).max(1) as f64;
            let mut svg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1000\" height=\"520\" viewBox=\"0 0 1000 520\"><rect width=\"1000\" height=\"520\" fill=\"#101827\"/><g font-family=\"sans-serif\" fill=\"#edf4ff\"><text x=\"30\" y=\"38\" font-size=\"24\">Perfil por instrumentación · tiempo exclusivo</text><text x=\"30\" y=\"65\" font-size=\"14\">Incluye coste del instrumento. No es un flamegraph de muestreo.</text>");
            for (i, (name, e)) in s.entries.iter().enumerate() {
                let inclusive = e.inclusive as f64 / 1e6;
                let exclusive = e.exclusive as f64 / 1e6;
                csv.push_str(&format!("{name},{},{inclusive:.6},{exclusive:.6}\n", e.calls));
                let y = 95 + i * 55;
                let w = e.exclusive as f64 / max * 490.0;
                svg.push_str(&format!("<text x=\"30\" y=\"{}\" font-size=\"15\">{name}</text><rect x=\"270\" y=\"{y}\" width=\"{w:.2}\" height=\"24\" rx=\"3\" fill=\"#24c4b4\"/><text x=\"780\" y=\"{}\" font-size=\"14\">{exclusive:.3} ms</text>", y + 18, y + 18));
            }
            svg.push_str("</g></svg>");
            std::fs::write(directory.join("perfil.csv"), csv)?;
            std::fs::write(directory.join("perfil.svg"), svg)
        })
    }
}

#[cfg(feature = "profile")]
pub use enabled::*;

#[cfg(not(feature = "profile"))]
pub struct Guard;
#[cfg(not(feature = "profile"))]
#[inline(always)]
pub fn scope(_: &'static str) -> Guard {
    Guard
}
#[cfg(not(feature = "profile"))]
pub fn save(_: &std::path::Path) -> std::io::Result<()> {
    Err(std::io::Error::other("Recompile con --features profile"))
}
