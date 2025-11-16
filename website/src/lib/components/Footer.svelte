<script lang="ts">
  const currentYear = new Date().getFullYear();
</script>

<footer class="footer">
  <div class="footer-content">
    <div class="footer-section">
      <h3 class="footer-title">Stasis</h3>
      <p class="footer-description">
        A modern Wayland idle manager that knows when to step back.
      </p>
      <div class="social-links">
        <a href="https://github.com/saltnpepper97/stasis" target="_blank" rel="noopener noreferrer" aria-label="GitHub">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M12 0c-6.626 0-12 5.373-12 12 0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23.957-.266 1.983-.399 3.003-.404 1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576 4.765-1.589 8.199-6.086 8.199-11.386 0-6.627-5.373-12-12-12z"/>
          </svg>
        </a>
        <a href="https://discord.gg/Qjsd8MjP" target="_blank" rel="noopener noreferrer" aria-label="Discord">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515a.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0a12.64 12.64 0 0 0-.617-1.25a.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057a19.9 19.9 0 0 0 5.993 3.03a.078.078 0 0 0 .084-.028a14.09 14.09 0 0 0 1.226-1.994a.076.076 0 0 0-.041-.106a13.107 13.107 0 0 1-1.872-.892a.077.077 0 0 1-.008-.128a10.2 10.2 0 0 0 .372-.292a.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127a12.299 12.299 0 0 1-1.873.892a.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028a19.839 19.839 0 0 0 6.002-3.03a.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.956-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419c0-1.333.955-2.419 2.157-2.419c1.21 0 2.176 1.096 2.157 2.42c0 1.333-.946 2.418-2.157 2.418z"/>
          </svg>
        </a>
      </div>
    </div>

    <div class="footer-section">
      <h4>Documentation</h4>
      <ul>
        <li><a href="/quick-start">Quick Start</a></li>
        <li><a href="/configuration">Configuration</a></li>
        <li><a href="/integration">Integration</a></li>
        <li><a href="/faq">FAQ</a></li>
      </ul>
    </div>

    <div class="footer-section">
      <h4>Community</h4>
      <ul>
        <li><a href="https://github.com/saltnpepper97/stasis" target="_blank" rel="noopener noreferrer">GitHub</a></li>
        <li><a href="https://github.com/saltnpepper97/stasis/issues" target="_blank" rel="noopener noreferrer">Report Issues</a></li>
        <li><a href="/contributing">Contributing</a></li>
        <li><a href="https://discord.gg/your-invite" target="_blank" rel="noopener noreferrer">Discord Server</a></li>
      </ul>
    </div>

    <div class="footer-section">
      <h4>Resources</h4>
      <ul>
        <li><a href="https://github.com/saltnpepper97/stasis/releases" target="_blank" rel="noopener noreferrer">Releases</a></li>
        <li><a href="https://aur.archlinux.org/packages/stasis" target="_blank" rel="noopener noreferrer">AUR Package</a></li>
        <li><a href="https://github.com/saltnpepper97/stasis/blob/main/LICENSE" target="_blank" rel="noopener noreferrer">License</a></li>
      </ul>
    </div>
  </div>

  <div class="footer-bottom">
    <p>© {currentYear} Stasis. Released under the MIT License.</p>
    <p class="made-with">Made with 💜 for the Wayland community</p>
  </div>
</footer>

<style>
  .footer {
    background: linear-gradient(135deg, rgba(168, 85, 247, 0.05) 0%, rgba(201, 42, 42, 0.05) 100%);
    border-top: 1px solid var(--border-color);
    padding: 60px 20px 20px;
    margin-top: auto;
  }

  .footer-content {
    max-width: 1200px;
    margin: 0 auto;
    display: grid;
    grid-template-columns: 2fr 1fr 1fr 1fr;
    gap: 48px;
    margin-bottom: 48px;
  }

  .footer-section h3 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 12px 0;
    background: linear-gradient(135deg, #a855f7 0%, #c92a2a 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .footer-section h4 {
    font-family: 'Space Grotesk', sans-serif;
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 16px 0;
    color: var(--text-primary);
  }

  .footer-description {
    color: var(--text-secondary);
    line-height: 1.6;
    margin: 0 0 20px 0;
    max-width: 300px;
  }

  .social-links {
    display: flex;
    gap: 12px;
  }

  .social-links a {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 50%;
    color: var(--text-primary);
    transition: all 0.2s ease;
  }

  .social-links a:hover {
    background: var(--accent);
    color: white;
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .footer-section ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .footer-section li {
    margin-bottom: 12px;
  }

  .footer-section a {
    color: var(--text-secondary);
    text-decoration: none;
    transition: color 0.2s ease;
    font-size: 0.95rem;
  }

  .footer-section a:hover {
    color: var(--accent);
  }

  .footer-bottom {
    max-width: 1200px;
    margin: 0 auto;
    padding-top: 24px;
    border-top: 1px solid var(--border-color);
    text-align: center;
  }

  .footer-bottom p {
    color: var(--text-secondary);
    font-size: 0.9rem;
    margin: 8px 0;
  }

  .made-with {
    font-size: 0.85rem !important;
  }

  @media (max-width: 968px) {
    .footer {
      padding: 40px 20px 20px;
    }

    .footer-content {
      grid-template-columns: 1fr;
      gap: 32px;
      margin-bottom: 32px;
    }

    .footer-description {
      max-width: 100%;
    }
  }
</style>
