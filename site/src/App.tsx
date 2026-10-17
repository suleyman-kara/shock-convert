import { useEffect, useState } from "react";
import { REPO, T as t } from "./i18n";

type Latest = { version: string; setup: string; sha256?: string };

/** Yayınlanmış en son sürüm: publish-download iş akışının yazdığı download/latest.json. */
function useLatest(): Latest {
  const fallback: Latest = {
    version: __APP_VERSION__,
    setup: `shock-convert-setup-${__APP_VERSION__}.exe`,
  };
  const [latest, setLatest] = useState<Latest>(fallback);
  useEffect(() => {
    fetch("download/latest.json", { cache: "no-cache" })
      .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
      .then((j: Latest) => {
        if (j && /^\d+\.\d+\.\d+$/.test(j.version) && /^[\w.-]+\.exe$/.test(j.setup)) setLatest(j);
      })
      .catch(() => {
        /* yerel geliştirmede ya da ilk yayından önce: derleme sürümü kullanılır */
      });
  }, []);
  return latest;
}

function Logo({ size = 32 }: { size?: number }) {
  return <img src="icon.png" width={size} height={size} alt="" className="logo" />;
}

function ContextMenuDemo() {
  return (
    <div className="demo" aria-hidden="true">
      <div className="file">
        <div className="file-thumb">
          <span>PNG</span>
        </div>
        <div className="file-name">{t.menu.file}</div>
      </div>
      <div className="menu">
        <div className="menu-item">{t.menu.open}</div>
        <div className="menu-item">{t.menu.copy}</div>
        <div className="menu-item">{t.menu.rename}</div>
        <div className="menu-sep" />
        <div className="menu-item menu-active">
          <Logo size={16} />
          <span>Shock Convert</span>
          <span className="chev">›</span>
          <div className="submenu">
            {["JPG", "WebP", "ICO", "BMP", "TIFF"].map((f, i) => (
              <div key={f} className={"menu-item" + (i === 0 ? " pick" : "")}>
                {f}
              </div>
            ))}
          </div>
        </div>
        <div className="menu-sep" />
        <div className="menu-item">{t.menu.props}</div>
      </div>
      <div className="toast">
        <Logo size={18} />
        <span>{t.menu.done}</span>
      </div>
    </div>
  );
}

export default function App() {
  const latest = useLatest();
  const dir = `download/v${latest.version}/`;
  const setupUrl = dir + latest.setup;
  const portableUrl = `${REPO}/releases/download/v${latest.version}/shock-convert.exe`;

  return (
    <>
      <header className="nav">
        <div className="wrap nav-inner">
          <a href="#top" className="brand">
            <Logo size={28} />
            <span>Shock Convert</span>
          </a>
          <nav className="nav-links">
            <a href="#features">{t.nav.features}</a>
            <a href="#how">{t.nav.how}</a>
            <a href="#download">{t.nav.download}</a>
            <a href="#faq">{t.nav.faq}</a>
          </nav>
          <div className="nav-actions">
            <a className="gh" href={REPO} aria-label="GitHub">
              <svg viewBox="0 0 16 16" width="20" height="20" aria-hidden="true">
                <path
                  fill="currentColor"
                  d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"
                />
              </svg>
            </a>
          </div>
        </div>
      </header>

      <main id="top">
        <section className="hero">
          <div className="wrap hero-inner">
            <div className="hero-copy">
              <span className="badge">{t.hero.badge}</span>
              <h1>
                {t.hero.title1}
                <br />
                <span className="accent">{t.hero.title2}</span>
              </h1>
              <p className="lead">{t.hero.lead}</p>
              <div className="cta-row">
                <a className="btn btn-primary" href={setupUrl}>
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
                    <path fill="currentColor" d="M12 16l-5-5h3V4h4v7h3l-5 5zm-7 2h14v2H5v-2z" />
                  </svg>
                  {t.hero.cta}
                  <span className="ver">v{latest.version}</span>
                </a>
                <a className="btn btn-ghost" href={REPO}>
                  {t.hero.secondary}
                </a>
              </div>
            </div>
            <ContextMenuDemo />
          </div>
        </section>

        <section id="features" className="section">
          <div className="wrap">
            <h2>{t.features.title}</h2>
            <div className="grid">
              {t.features.items.map((f) => (
                <article key={f.t} className="card">
                  <div className="card-icon" aria-hidden="true">
                    {f.icon}
                  </div>
                  <h3>{f.t}</h3>
                  <p>{f.d}</p>
                </article>
              ))}
            </div>
          </div>
        </section>

        <section id="how" className="section alt">
          <div className="wrap">
            <h2>{t.how.title}</h2>
            <ol className="steps">
              {t.how.steps.map((s, i) => (
                <li key={s.t}>
                  <span className="step-n">{i + 1}</span>
                  <h3>{s.t}</h3>
                  <p>{s.d}</p>
                </li>
              ))}
            </ol>
            <p className="muted center">{t.how.note}</p>
          </div>
        </section>

        <section id="download" className="section">
          <div className="wrap">
            <h2>{t.download.title}</h2>
            <div className="download">
              <div className="dl-main">
                <a className="dl-card primary" href={setupUrl}>
                  <span className="dl-title">{t.download.setup}</span>
                  <span className="dl-hint">{t.download.setupHint}</span>
                  <span className="dl-meta">
                    {t.download.version} {latest.version} · x64
                  </span>
                </a>
                <a className="dl-card" href={portableUrl}>
                  <span className="dl-title">{t.download.portable}</span>
                  <span className="dl-hint">{t.download.portableHint}</span>
                </a>
                <div className="dl-links">
                  <a href={`${REPO}/releases`}>{t.download.all}</a>
                  <a href={dir + "SHA256SUMS.txt"}>{t.download.checksum}</a>
                </div>
                {latest.sha256 && (
                  <code className="sha" title="SHA-256">
                    {latest.sha256}
                  </code>
                )}
              </div>
              <div className="dl-side">
                <h3>{t.download.reqTitle}</h3>
                <ul>
                  {t.download.req.map((r) => (
                    <li key={r}>{r}</li>
                  ))}
                </ul>
                <div className="notice">
                  <strong>{t.download.smartTitle}</strong>
                  <p>{t.download.smart}</p>
                </div>
              </div>
            </div>
          </div>
        </section>

        <section className="section alt">
          <div className="wrap">
            <h2>{t.trust.title}</h2>
            <div className="grid three">
              {t.trust.items.map((i) => (
                <article key={i.t} className="card plain">
                  <h3>{i.t}</h3>
                  <p>{i.d}</p>
                </article>
              ))}
            </div>
          </div>
        </section>

        <section id="faq" className="section">
          <div className="wrap narrow">
            <h2>{t.faq.title}</h2>
            {t.faq.items.map((f) => (
              <details key={f.q} className="faq">
                <summary>{f.q}</summary>
                <p>{f.a}</p>
              </details>
            ))}
          </div>
        </section>
      </main>

      <footer className="footer">
        <div className="wrap footer-inner">
          <div className="brand small">
            <Logo size={22} />
            <span>Shock Convert</span>
          </div>
          <nav>
            <a href={`${REPO}/blob/main/LICENSE`}>{t.footer.license}</a>
            <a href={`${REPO}/releases`}>{t.footer.releases}</a>
            <a href={`${REPO}/issues`}>{t.footer.issues}</a>
          </nav>
        </div>
      </footer>
    </>
  );
}
