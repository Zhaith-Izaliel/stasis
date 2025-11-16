<script lang="ts">
  import { onMount } from 'svelte';
  
  let activeSection = $state('');
  
  const sections = [
    { id: 'prerequisites', title: 'Prerequisites' },
    { id: 'input-group', title: 'Input Group Setup' },
    { id: 'manual', title: 'Running Manually' },
    { id: 'systemd', title: 'Systemd Service' }
  ];
  
  onMount(() => {
    // Handle anchor links from other pages
    const hash = window.location.hash.slice(1);
    if (hash) {
      const element = document.getElementById(hash);
      if (element) {
        const topbarHeight = window.innerWidth <= 768 ? 0 : 70;
        const elementPosition = element.getBoundingClientRect().top + window.scrollY;
        const offsetPosition = elementPosition - topbarHeight;
        
        window.scrollTo({
          top: offsetPosition,
          behavior: 'instant'
        });
      }
    }
    
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
      const topbarHeight = window.innerWidth <= 768 ? 80 : 70;
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
    <h1>Quick Start</h1>
    
    <section id="prerequisites">
      <h2>Prerequisites</h2>
      <div class="warning">
        <strong>⚠️ Important:</strong> Before running Stasis, you must be part of the <code>input</code> group.
      </div>
  
    </section>
    
    <section id="input-group">
      <h2>Input Group Setup</h2>
      <p>Check if you're already in the input group:</p>
      <pre><code>groups $USER</code></pre>
      <p>You should see output like:</p>
      <pre><code>dustin : dustin wheel audio input storage video</code></pre>
      <p>If <code>input</code> is missing, add yourself to the group:</p>
      <pre><code>sudo usermod -aG input $USER</code></pre>
      <p class="note">
        <strong>Note:</strong> You'll need to log out and back in or restart your computer for group changes to take effect.
      </p>

      <p class="note">
        <strong>Note:</strong> On first run, Stasis automatically generates a configuration file at 
        <code>$XDG_CONFIG_HOME/stasis/stasis.rune</code> (typically <code>~/.config/stasis/stasis.rune</code>).
      </p>
    </section>
    
    <section id="manual">
      <h2>Running Manually</h2>
      <p>
        Stasis must be started from within a running Wayland session. 
        Simply run:
      </p>
      <pre><code>stasis</code></pre>
      <p>
        This is useful for testing, but for daily use we recommend setting up 
        the systemd service below.
      </p>
    </section>
    
    <section id="systemd">
      <h2>Systemd Service (Recommended)</h2>
      <p>
        For automatic startup, create a systemd user service at 
        <code>~/.config/systemd/user/stasis.service</code>:
      </p>
      <pre><code>[Unit]
Description=Stasis Wayland Idle Manager
After=graphical-session.target
Wants=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/stasis
Restart=always
RestartSec=5
Environment=WAYLAND_DISPLAY=wayland-0
# Optional: wait until WAYLAND_DISPLAY exists
ExecStartPre=/bin/sh -c 'while [ ! -e /run/user/%U/wayland-0 ]; do sleep 0.1; done'

[Install]
WantedBy=default.target</code></pre>
      
      <p>Enable and start the service:</p>
      <pre><code>systemctl --user enable stasis.service
systemctl --user start stasis.service</code></pre>
      
      <p>Check the service status:</p>
      <pre><code>systemctl --user status stasis.service</code></pre>
    </section>
  </main>
</div>

<style>
  .page-container {
    display: grid;
    grid-template-columns: 200px 1fr;
    gap: 40px;
    max-width: 1200px;
    margin: 0 auto;
    padding: 40px 20px;
  }
  
  .links-nav {
    position: sticky;
    top: 80px;
    height: fit-content;
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
    padding: 0;
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
    padding: 6px 0 6px 16px;
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
    scroll-margin-top: 120px;
  }
 
  section {
    margin-bottom: 48px;
    scroll-margin-top: 120px;
  }
 
  p {
    line-height: 1.7;
    color: var(--text-primary);
    margin: 16px 0;
  }
  
  .warning {
    background: rgba(255, 193, 7, 0.1);
    border-left: 4px solid #ffc107;
    padding: 16px;
    margin: 24px 0;
    border-radius: 4px;
  }
  
  .note {
    background: var(--bg-secondary);
    border-left: 4px solid var(--accent);
    padding: 16px;
    margin: 24px 0;
    border-radius: 4px;
  }
  
  code {
    background: var(--bg-secondary);
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
    font-size: 0.9em;
    color: var(--text-primary);
    word-break: break-all;
  }
  
  pre {
    background: var(--bg-secondary);
    padding: 16px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 16px 0;
    border: 1px solid var(--border-color);
  }
  
  pre code {
    background: none;
    padding: 0;
    font-size: 0.9rem;
    word-break: normal;
  }
  
  @media (max-width: 768px) {
    .page-container {
      grid-template-columns: 1fr;
      gap: 20px;
      padding: 80px 16px 20px; /* Extra top padding for hamburger menu */
    }
    
    .links-nav {
      position: static;
      border-bottom: 1px solid var(--border-color);
      padding-bottom: 16px;
      margin-bottom: 8px;
    }
    
    .nav-title {
      font-size: 0.8rem;
      margin-bottom: 10px;
    }
    
    .links-nav ul {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: 8px;
    }
    
    .links-nav button {
      border-left: none;
      border-bottom: 2px solid transparent;
      padding: 10px 12px;
      font-size: 0.8rem;
      background: var(--bg-secondary);
      border-radius: 6px;
      text-align: center;
    }
    
    .links-nav button.active {
      border-bottom-color: var(--accent);
      border-left: none;
      background: rgba(168, 85, 247, 0.1);
    }
    
    h1 {
      font-size: 2rem;
      margin-bottom: 24px;
    }
    
    h2 {
      font-size: 1.4rem;
      margin: 32px 0 12px 0;
      scroll-margin-top: 100px; /* Account for hamburger menu */
    }
    
    section {
      margin-bottom: 32px;
      scroll-margin-top: 100px; /* Account for hamburger menu */
    }
    
    p {
      font-size: 0.95rem;
    }
    
    .warning,
    .note {
      padding: 12px;
      font-size: 0.9rem;
    }
    
    code {
      font-size: 0.85em;
    }
    
    pre {
      padding: 12px;
      font-size: 0.8rem;
      margin: 12px 0;
    }
    
    pre code {
      font-size: 0.8rem;
    }
  }
  
  /* Extra small devices */
  @media (max-width: 480px) {
    .page-container {
      padding: 70px 12px 20px;
    }
    
    .links-nav ul {
      grid-template-columns: 1fr;
    }
    
    .links-nav button {
      padding: 8px 10px;
      font-size: 0.75rem;
    }
    
    h1 {
      font-size: 1.75rem;
    }
    
    h2 {
      font-size: 1.25rem;
    }
  }
</style>
