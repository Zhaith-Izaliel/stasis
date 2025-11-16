<script lang="ts">
  import { base } from '$app/paths';
  
  import stasisLogo from '$lib/assets/stasis.png';
</script>

<div class="hero">
  <div class="hero-content">
    <img src={stasisLogo} alt="Stasis Logo" class="logo" />
    <h1>Stasis</h1>
    <p class="tagline">A modern Wayland idle manager that knows when to step back.</p>
    <p class="subtitle">Keep your session in perfect balance—automatically preventing idle when it matters, allowing it when it doesn't.</p>
    
    <div class="cta-buttons">
      <a href="{base}/quick-start" class="btn btn-primary">Get Started</a>
      <a href="https://github.com/saltnpepper97/stasis" class="btn btn-secondary" target="_blank" rel="noopener noreferrer">
        <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
        </svg>
        View on GitHub
      </a>
    </div>

    <div class="discord-banner">
      <span class="discord-icon">💬</span>
      <span>Join the community on</span>
      <a href="https://discord.gg/Qjsd8MjP" target="_blank" rel="noopener noreferrer">Discord</a>
    </div>
  </div>
</div>

<section class="features">
  <div class="feature-grid">
    <div class="feature-card">
      <div class="feature-icon">🧠</div>
      <h3>Context-Aware</h3>
      <p>Watching a video? Reading a document? Stasis understands your activity and prevents unwanted screen locks automatically.</p>
    </div>

    <div class="feature-card">
      <div class="feature-icon">🎵</div>
      <h3>Media Detection</h3>
      <p>Automatically detects media playback and honors Wayland idle inhibitor protocols—no more mouse jiggles during movies.</p>
    </div>

    <div class="feature-card">
      <div class="feature-icon">⚙️</div>
      <h3>Flexible Actions</h3>
      <p>Configure custom actions with regex patterns, application-specific rules, and named action blocks for complete control.</p>
    </div>

    <div class="feature-card">
      <div class="feature-icon">⚡</div>
      <h3>Live Reload</h3>
      <p>Update your configuration on the fly without restarting. Changes take effect immediately.</p>
    </div>

    <div class="feature-card">
      <div class="feature-icon">🛌</div>
      <h3>Lid Event Handling</h3>
      <p>Detects laptop lid open/close events via DBus to intelligently manage idle behavior.</p>
    </div>

    <div class="feature-card">
      <div class="feature-icon">📝</div>
      <h3>Clean Config</h3>
      <p>Uses the intuitive RUNE configuration language for easy setup and maintenance.</p>
    </div>
  </div>
</section>

<style>
  .hero {
    min-height: calc(100vh - 70px);
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, rgba(168, 85, 247, 0.05) 0%, rgba(201, 42, 42, 0.05) 100%);
    padding: 60px 20px;
  }

  .hero-content {
    max-width: 800px;
    text-align: center;
  }

  .logo {
    width: 180px;
    height: 180px;
    margin: 0 auto 32px;
    display: block;
    filter: drop-shadow(0 4px 12px rgba(168, 85, 247, 0.3));
    animation: float 3s ease-in-out infinite;
  }

  @keyframes float {
    0%, 100% {
      transform: translateY(0px);
    }
    50% {
      transform: translateY(-10px);
    }
  }

  h1 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 4.5rem;
    font-weight: 700;
    margin: 0 0 24px 0;
    background: linear-gradient(135deg, #a855f7 0%, #c92a2a 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
    letter-spacing: -2px;
  }

  .tagline {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 1.5rem;
    font-weight: 500;
    color: var(--text-primary);
    margin: 0 0 16px 0;
    line-height: 1.4;
  }

  .subtitle {
    font-size: 1.1rem;
    color: var(--text-secondary);
    margin: 0 0 40px 0;
    line-height: 1.6;
    max-width: 600px;
    margin-left: auto;
    margin-right: auto;
  }

  .cta-buttons {
    display: flex;
    gap: 16px;
    justify-content: center;
    margin-bottom: 32px;
  }

  .btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 14px 32px;
    border-radius: 8px;
    font-family: 'Space Grotesk', sans-serif;
    font-weight: 600;
    font-size: 1.05rem;
    text-decoration: none;
    transition: all 0.2s ease;
    border: none;
    cursor: pointer;
  }

  .btn-primary {
    background: linear-gradient(135deg, #a855f7 0%, #c92a2a 100%);
    color: white;
    box-shadow: 0 4px 12px rgba(168, 85, 247, 0.3);
  }

  .btn-primary:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 16px rgba(168, 85, 247, 0.4);
  }

  .btn-secondary {
    background: var(--bg-secondary);
    color: var(--text-primary);
    border: 2px solid var(--border-color);
  }

  .btn-secondary:hover {
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .discord-banner {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 12px 24px;
    background: rgba(88, 101, 242, 0.1);
    border: 1px solid rgba(88, 101, 242, 0.3);
    border-radius: 8px;
    font-size: 0.95rem;
    color: var(--text-primary);
  }

  .discord-icon {
    font-size: 1.2rem;
  }

  .discord-banner a {
    color: #5865f2;
    text-decoration: none;
    font-weight: 600;
    transition: opacity 0.2s ease;
  }

  .discord-banner a:hover {
    opacity: 0.8;
    text-decoration: underline;
  }

  .features {
    padding: 80px 20px;
    max-width: 1200px;
    margin: 0 auto;
  }

  .feature-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 32px;
  }

  .feature-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 32px;
    transition: all 0.3s ease;
  }

  .feature-card:hover {
    transform: translateY(-4px);
    border-color: var(--accent);
    box-shadow: 0 8px 24px rgba(168, 85, 247, 0.15);
  }

  .feature-icon {
    font-size: 3rem;
    margin-bottom: 16px;
  }

  .feature-card h3 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 1.4rem;
    font-weight: 600;
    margin: 0 0 12px 0;
    color: var(--text-primary);
  }

  .feature-card p {
    font-size: 0.95rem;
    line-height: 1.6;
    color: var(--text-secondary);
    margin: 0;
  }

  .badges {
    padding: 40px 20px 80px;
    text-align: center;
  }

  .badge-container {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    justify-content: center;
    align-items: center;
  }

  .badge-container img {
    height: 20px;
  }

  @media (max-width: 768px) {
    .hero {
      min-height: auto;
      padding: 40px 20px;
    }

    .logo {
      width: 120px;
      height: 120px;
      margin-bottom: 24px;
    }

    h1 {
      font-size: 3rem;
      letter-spacing: -1px;
    }

    .tagline {
      font-size: 1.2rem;
    }

    .subtitle {
      font-size: 1rem;
    }

    .cta-buttons {
      flex-direction: column;
      align-items: stretch;
    }

    .btn {
      justify-content: center;
    }

    .discord-banner {
      flex-wrap: wrap;
      justify-content: center;
    }

    .features {
      padding: 40px 20px;
    }

    .feature-grid {
      grid-template-columns: 1fr;
      gap: 24px;
    }

    .feature-card {
      padding: 24px;
    }
  }
</style>
