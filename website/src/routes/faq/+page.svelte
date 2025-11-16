<script lang="ts">
  import { onMount } from 'svelte';
  
  let activeSection = $state('');
  
  const sections = [
    { id: 'waybar', title: 'Waybar' }
  ];
  
  onMount(() => {
    // Handle anchor links from other pages
    const hash = window.location.hash.slice(1);
    if (hash) {
      const element = document.getElementById(hash);
      if (element) {
        const topbarHeight = window.innerWidth <= 968 ? 0 : 70;
        const elementPosition = element.getBoundingClientRect().top + window.scrollY;
        const offsetPosition = elementPosition - topbarHeight;
        
        window.scrollTo({
          top: offsetPosition,
          behavior: 'instant'
        });
      }
    }

    // Observe sections for active state
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            activeSection = entry.target.id;
          }
        });
      },
      { rootMargin: '-100px 0px -80% 0px' }
    );
    
    sections.forEach(({ id }) => {
      const element = document.getElementById(id);
      if (element) observer.observe(element);
    });
    
    return () => observer.disconnect();
  });
  
  function scrollToSection(id: string) {
    const element = document.getElementById(id);
    if (element) {
      const topbarHeight = window.innerWidth <= 968 ? 80 : 70;
      const elementPosition = element.getBoundingClientRect().top + window.scrollY;
      const offsetPosition = elementPosition - topbarHeight;
      
      window.scrollTo({
        top: offsetPosition,
        behavior: 'smooth'
      });
    }
  }
</script>


<div class="page-container">
  <nav class="links-nav">
    <div class="nav-title">On this page</div>
    <ul>
      {#each sections as section}
        <li>
          <button
            class:active={activeSection === section.id}
            onclick={() => scrollToSection(section.id)}
          >
            {section.title}
          </button>
        </li>
      {/each}
    </ul>
  </nav>
  
  <main class="content">
    <h1>Frequently Asked Questions</h1>
    
    <section id="app-detection">
      <h2>Stasis Not Detecting Apps</h2>
      <p>If Stasis isn't detecting your applications for idle inhibition:</p>
      <ul>
        <li>Ensure your compositor is supported (see Supported Compositors)</li>
        <li>Check that the app names in <code>inhibit_apps</code> match the actual application names</li>
        <li>Use <code>stasis -v</code> or check <code>~/.cache/stasis/stasis.log</code> for detailed logs on detected apps</li>
      </ul>
    </section>
    
    <section id="regex-patterns">
      <h2>Regex Patterns Not Matching</h2>
      <p>If your regex patterns in <code>inhibit_apps</code> aren't working:</p>
      <ul>
        <li>Ensure you're using raw string syntax: <code>r"pattern"</code></li>
        <li>Test patterns with verbose logging to see what apps are detected</li>
        <li>Remember that River uses process-based detection (fallback) which may have different app names</li>
      </ul>
      <div class="info">
        <strong>💡 Tip:</strong> Run Stasis with verbose logging (<code>stasis -v</code>) to see exactly what application names are being detected, then adjust your patterns accordingly.
      </div>
    </section>
    
    <section id="service-issues">
      <h2>Service Not Starting</h2>
      <p>If your systemd service won't start:</p>
      <ul>
        <li>Verify the <code>ExecStart</code> path in your systemd service file points to the correct binary location</li>
        <li>Check service logs for specific errors:</li>
      </ul>
      <pre><code>journalctl --user -u stasis.service</code></pre>
      <p>Common issues include incorrect binary paths or missing dependencies.</p>
    </section>
    
    <section id="config-reload">
      <h2>Configuration Not Reloading</h2>
      <p>If changes to your configuration aren't taking effect:</p>
      <ul>
        <li>Use <code>stasis reload</code> to send a reload signal to the running daemon</li>
        <li>Check configuration syntax if reload fails</li>
        <li>Restart the service if reload continues to fail:</li>
      </ul>
      <pre><code>systemctl --user restart stasis.service</code></pre>
    </section>
    
    <section id="config-errors">
      <h2>Configuration Errors</h2>
      <p>If Stasis reports configuration errors:</p>
      <ul>
        <li>Validate your RUNE syntax (see RUNE notes in documentation)</li>
        <li>Verify you're using the correct built-in action block names (they are fixed as of v0.1.2):
          <ul>
            <li><code>lock_screen</code> / <code>lock-screen</code></li>
            <li><code>dpms</code></li>
            <li><code>suspend</code></li>
            <li><code>brightness</code></li>
            <li><code>custom-*</code> (for custom actions)</li>
          </ul>
        </li>
        <li>Check the manual: <code>man 5 stasis</code></li>
        <li>Use verbose logging to identify configuration issues</li>
      </ul>
    </section>
    
    <section id="brightness">
      <h2>Brightness Not Correctly Resetting</h2>
      <p>If brightness controls aren't working properly:</p>
      <div class="warning">
        <strong>⚠️ Video Group Required:</strong>
        <p>Check the logs with <code>cat ~/.cache/stasis/stasis.log</code>. If you see warnings about setting brightness, you need to add yourself to the <code>video</code> group:</p>
        <pre><code>sudo gpasswd -a &lt;user&gt; video</code></pre>
        <p>Log out and back in for the group change to take effect.</p>
      </div>
    </section>
    
    <section id="input-timer">
      <h2>Input Timer Increasing Randomly</h2>
      <p>If the idle timer keeps counting up even while you're using your mouse or keyboard:</p>
      <div class="warning">
        <strong>⚠️ Input Group Required:</strong>
        <p>Ensure your user is in the <code>input</code> group:</p>
        <pre><code>sudo gpasswd -a &lt;user&gt; input</code></pre>
        <p>Log out and back in for the group change to take effect.</p>
      </div>
      <p>This is the most common cause of idle detection issues. See the <a href="/quick-start#prerequisites">Quick Start guide</a> for more details.</p>
    </section>
    
    <section id="help">
      <h2>Need More Help?</h2>
      <p>If your problem isn't listed here and you've tried everything:</p>
      <div class="info">
        <strong>🐛 Open a Bug Report</strong>
        <p>Visit the <a href="https://github.com/saltnpepper97/stasis/issues" target="_blank" rel="noopener noreferrer">GitHub Issues</a> page to report your problem. Please include:</p>
        <ul>
          <li>Your distribution and compositor</li>
          <li>Stasis version (<code>stasis --version</code>)</li>
          <li>Relevant log output from <code>~/.cache/stasis/stasis.log</code></li>
          <li>Your configuration file (sanitize any sensitive info)</li>
        </ul>
      </div>
    </section>
  </main>
</div>

<style>
/* === LAYOUT === */
.page-container {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 40px;
  max-width: 1400px;
  margin: 0 auto;
  padding: 40px 20px;
}

/* === SIDE NAVIGATION === */
.links-nav {
  position: sticky;
  top: 80px;
  height: fit-content;
  padding-top: 8px; /* FIX: consistent top padding like FAQ */
}

.nav-title {
  font-weight: 600;
  font-size: 0.85rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
  margin-bottom: 12px;
}

.links-nav ul {
  list-style: none;
  padding: 6px 0 0 0; /* padding to avoid flush anchor */
  margin: 0;
}

.links-nav li {
  margin: 0;
}

.links-nav button {
  display: block;
  width: 100%;
  text-align: left;
  background: none;
  border: none;
  border-left: 2px solid var(--border-color);
  padding: 8px 0 8px 16px;
  font-size: 0.9rem;
  color: var(--text-secondary);
  cursor: pointer;
  transition: all 0.2s ease;
}

.links-nav button:hover {
  color: var(--text-primary);
  border-left-color: var(--accent);
}

.links-nav button.active {
  color: var(--accent);
  border-left-color: var(--accent);
  font-weight: 500;
}

/* === CONTENT === */
.content {
  min-width: 0;
}

h1 {
  font-size: 2.5rem;
  font-weight: 700;
  margin: 0 0 32px 0;
  color: var(--text-primary);
}

h2 {
  font-size: 1.75rem;
  font-weight: 600;
  margin: 48px 0 16px 0;
  color: var(--text-primary);
  scroll-margin-top: 70px;
}

section {
  margin-bottom: 48px;
  scroll-margin-top: 70px;
}

p, li {
  line-height: 1.7;
  color: var(--text-primary);
}

ul {
  line-height: 1.8;
  color: var(--text-primary);
  margin: 16px 0;
  padding-left: 24px;
}

li {
  margin: 8px 0;
}

a {
  color: var(--accent);
  text-decoration: none;
  transition: opacity 0.2s ease;
}

a:hover {
  opacity: 0.8;
  text-decoration: underline;
}

/* === BLOCKS === */
.warning,
.info {
  border-left: 4px solid;
  padding: 20px;
  margin: 24px 0;
  border-radius: 4px;
}

.warning {
  background: rgba(255, 193, 7, 0.1);
  border-left-color: #ffc107;
}

.info {
  background: var(--bg-secondary);
  border-left-color: var(--accent);
}

.warning strong,
.info strong {
  display: block;
  margin-bottom: 12px;
  font-size: 1.05rem;
}

code {
  background: var(--bg-secondary);
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 0.9em;
  color: var(--text-primary);
  word-break: break-word;
}

pre {
  background: var(--bg-secondary);
  padding: 20px;
  border-radius: 6px;
  overflow-x: auto;
  margin: 20px 0;
  border: 1px solid var(--border-color);
  line-height: 1.5;
}

/* === MOBILE: unify and simplify === */
@media (max-width: 968px) {
  .page-container {
    grid-template-columns: 1fr;
    gap: 20px;
    padding: 80px 16px 20px; /* hamburger space */
  }

  .links-nav {
    position: static;
    border-bottom: 1px solid var(--border-color);
    padding-top: 12px;
    padding-bottom: 16px;
    margin-bottom: 8px;
  }

  .nav-title {
    font-size: 0.8rem;
    margin-bottom: 10px;
  }

  .links-nav ul {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 0;
  }

  .links-nav button {
    border-left: none;
    border-bottom: 2px solid transparent;
    padding: 8px 12px;
    font-size: 0.85rem;
    background: var(--bg-secondary);
    border-radius: 6px;
    text-align: center;
  }

  .links-nav button.active {
    border-bottom-color: var(--accent);
    background: rgba(168, 85, 247, 0.1);
  }

  h1 {
    font-size: 2rem;
    margin-bottom: 24px;
  }

  h2 {
    font-size: 1.5rem;
    margin: 32px 0 12px 0;
    scroll-margin-top: 100px;
  }

  section {
    margin-bottom: 32px;
    scroll-margin-top: 100px;
  }

  p, pre, ul {
    font-size: 0.95rem;
  }

  pre {
    padding: 12px;
    font-size: 0.8rem;
  }
}

/* === TINY DEVICES === */
@media (max-width: 480px) {
  .links-nav ul {
    flex-direction: column;
  }

  .links-nav button {
    font-size: 0.75rem;
    padding: 8px 10px;
  }

  h1 {
    font-size: 1.75rem;
  }

  h2 {
    font-size: 1.25rem;
  }

  pre {
    font-size: 0.75rem;
    padding: 10px;
  }
}

</style>
