<script lang="ts">
  import { onMount } from 'svelte';
  
  let activeSection = $state('');
  
  const sections = [
    { id: 'overview', title: 'Overview' },
    { id: 'global', title: 'Global Settings' },
    { id: 'stasis-block', title: 'Stasis Block' },
    { id: 'media', title: 'Media Monitoring' },
    { id: 'inhibitors', title: 'Inhibitors' },
    { id: 'laptop', title: 'Laptop Settings' },
    { id: 'actions', title: 'Idle Actions' },
    { id: 'desktop', title: 'Desktop Actions' },
    { id: 'ac-battery', title: 'AC & Battery' },
    { id: 'example', title: 'Full Example' }
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
    <h1>Configuration</h1>
    
    <section id="overview">
      <h2>Overview</h2>
      <p>
        Stasis uses a <code>.rune</code> configuration file located at 
        <code>$XDG_CONFIG_HOME/stasis/stasis.rune</code> (typically <code>~/.config/stasis/stasis.rune</code>).
        On first run, Stasis automatically generates a default configuration file with sensible defaults.
      </p>
      <p>
        The default configuration template is located at <code>/etc/stasis/stasis.rune</code>.
      </p>
      <p>
        The configuration is structured hierarchically with a main <code>stasis:</code> block containing
        all settings, actions, and laptop-specific power profiles.
      </p>
    </section>
    
    <section id="global">
      <h2>Global Settings</h2>
      <p>At the top of your config, you can define global variables and metadata:</p>
      <pre><code>@author "Your Name"
@description "Stasis configuration file"

# Define global variables for reuse
default_timeout 300  # 5 minutes</code></pre>
      <p>
        Global variables can be referenced throughout your configuration, making it easier to maintain consistent values.
      </p>
    </section>
    
    <section id="stasis-block">
      <h2>Stasis Block</h2>
      <p>All configuration lives within the <code>stasis:</code> block:</p>
      <pre><code>stasis:
  pre_suspend_command "hyprlock"
  monitor_media true
  ignore_remote_media true
  respect_idle_inhibitors true
  
  # ... action blocks and laptop configs
end</code></pre>
      
      <h3>Pre-Suspend Command</h3>
      <p>
        <code>pre_suspend_command</code> runs before the system suspends. Commonly used to lock the screen:
      </p>
      <pre><code>pre_suspend_command "hyprlock"</code></pre>
    </section>
    
    <section id="media">
      <h2>Media Monitoring</h2>
      
      <h3>Media Playback</h3>
      <p>Control whether Stasis monitors media playback to prevent idle actions:</p>
      <pre><code>monitor_media true
ignore_remote_media true</code></pre>
      <ul>
        <li><code>monitor_media</code> - When true, active media playback prevents idle timeouts</li>
        <li><code>ignore_remote_media</code> - Ignores media from remote sources (KDEConnect, Spotify remote play, etc.)</li>
      </ul>
      
      <h3>Media Blacklist</h3>
      <p>Ignore specific media players when checking for active playback:</p>
      <pre><code>media_blacklist ["spotify", "rhythmbox"]</code></pre>
    </section>
    
    <section id="inhibitors">
      <h2>Inhibitors</h2>
      
      <h3>Idle Inhibitors</h3>
      <p>Respect system-wide idle inhibitors from other applications:</p>
      <pre><code>respect_idle_inhibitors true</code></pre>
      
      <h3>Application Inhibitors</h3>
      <p>
        Specify applications that should prevent idle actions when active.
        Supports exact names and regex patterns:
      </p>
      <pre><code>inhibit_apps [
  "vlc"
  "Spotify"
  "mpv"
  r".*\.exe"           # Any .exe (Wine/Proton)
  r"steam_app_.*"      # Steam games
  r"firefox.*"         # Firefox windows
]</code></pre>
      
      <div class="info">
        <strong>Regex Pattern Guidelines:</strong>
        <ul>
          <li>Use raw string syntax: <code>r"pattern"</code> for all regex patterns</li>
          <li>Escape special characters properly: <code>\.</code> for literal dots, <code>\d+</code> for digits</li>
          <li>Use <code>.*</code> for wildcard matching</li>
          <li>Use <code>^</code> for start-of-string and <code>$</code> for end-of-string anchors</li>
          <li>Test your patterns with verbose logging to ensure they match correctly</li>
          <li><strong>NOTE:</strong> Stasis uses the <code>regex</code> crate for pattern matching</li>
        </ul>
      </div>
    </section>
    
    <section id="laptop">
      <h2>Laptop Settings</h2>
      
      <h3>Lid Actions</h3>
      <p>Configure what happens when the laptop lid closes or opens:</p>
      <pre><code>lid_close_action "lock-screen"
lid_open_action "wake"</code></pre>
      
      <div class="info">
        <strong>Available lid_close_action options:</strong>
        <ul>
          <li><code>lock-screen</code> - Lock the screen</li>
          <li><code>suspend</code> - Suspend the system</li>
          <li><code>custom</code> - Run a custom command</li>
          <li><code>ignore</code> - Do nothing</li>
        </ul>
      </div>
      
      <div class="info">
        <strong>Available lid_open_action options:</strong>
        <ul>
          <li><code>wake</code> - Wake the system</li>
          <li><code>custom</code> - Run a custom command</li>
          <li><code>ignore</code> - Do nothing</li>
        </ul>
      </div>
      
      <h3>Debounce</h3>
      <p>
        Prevent rapid lid open/close events from triggering multiple actions.
        Default is 3 seconds:
      </p>
      <pre><code>debounce_seconds 4</code></pre>
    </section>
    
    <section id="actions">
      <h2>Idle Actions</h2>
      <p>
        Stasis supports several built-in action types that trigger after periods of inactivity.
        Each action block has three key parameters:
      </p>
      <ul>
        <li><code>timeout</code> - Seconds of idle time before triggering (required)</li>
        <li><code>command</code> - Command to run when action triggers (required)</li>
        <li><code>resume-command</code> - Command to run when activity resumes (optional)</li>
      </ul>
      
      <h3>Built-in Action Types</h3>
      <ul>
        <li><code>lock_screen</code> / <code>lock-screen</code> - Lock the session</li>
        <li><code>dpms</code> - Display power management (screen off)</li>
        <li><code>suspend</code> - System suspend</li>
        <li><code>brightness</code> - Adjust screen brightness (laptop only)</li>
        <li><code>custom-*</code> - Custom actions with any name</li>
      </ul>
    </section>
    
    <section id="desktop">
      <h2>Desktop Actions</h2>
      <p>
        Desktop actions apply to all devices (desktops and laptops when not in AC/battery profiles).
        Define them directly under the <code>stasis:</code> block:
      </p>
      <pre><code>lock_screen:
  timeout 300  # 5 minutes
  command "loginctl lock-session"
  resume-command "notify-send 'Welcome Back $env.USER!'"
end

dpms:
  timeout 60  # 1 minute after lock
  command "niri msg action power-off-monitors"
  resume-command "niri msg action power-on-monitors"
end

suspend:
  timeout 1800  # 30 minutes
  command "systemctl suspend"
  resume-command None
end</code></pre>
      
      <div class="warning">
        <strong>🔒 loginctl Integration:</strong>
        When using <code>loginctl lock-session</code> as your lock command, you <b>MUST</b> specify 
        the actual locker via the <code>lock-command</code> parameter:
        <pre><code>lock_screen:
  timeout 300
  command "loginctl lock-session"
  lock-command "swaylock"  # REQUIRED when using loginctl
end</code></pre>
        <p>
          The <code>lock-command</code> is <b>required</b> when <code>command</code> 
          is set to <code>loginctl lock-session</code>. This tells loginctl which locker to use
          when managing the lock state.
        </p>
        <p>
          <b>Note:</b> You can lock your session at any time with 
          <code>loginctl lock-session</code> even when using Stasis without needing 
          <code>lock-command</code> in the config.
        </p>
      </div>
    </section>
    
    <section id="ac-battery">
      <h2>AC & Battery Profiles</h2>
      <p>
        Laptops can define separate action profiles for AC power and battery power.
        These override desktop actions when applicable.
      </p>
      
      <h3>AC Power Profile</h3>
      <p>Actions when plugged in:</p>
      <pre><code>on_ac:
  # Instant action (0 second timeout)
  custom-brightness-instant:
    timeout 0
    command "brightnessctl set 100%"
  end
  
  brightness:
    timeout 120  # 2 minutes
    command "brightnessctl set 30%"
  end
  
  dpms:
    timeout 60
    command "niri msg action power-off-monitors"
  end
  
  lock_screen:
    timeout 120
    command "swaylock"
  end
  
  suspend:
    timeout 300
    command "systemctl suspend"
  end
end</code></pre>
      
      <h3>Battery Profile</h3>
      <p>More aggressive timeouts to save battery:</p>
      <pre><code>on_battery:
  custom-brightness-instant:
    timeout 0
    command "brightnessctl set 40%"
  end
  
  brightness:
    timeout 60  # 1 minute
    command "brightnessctl set 20%"
  end
  
  dpms:
    timeout 30  # 30 seconds
    command "niri msg action power-off-monitors"
    resume-command "niri msg action power-on-monitors"
  end
  
  lock_screen:
    timeout 120  # 2 minutes
    command "swaylock"
    resume-command "notify-send 'Welcome back $env.USER!'"
  end
  
  suspend:
    timeout 120  # 2 minutes
    command "systemctl suspend"
  end
end</code></pre>
      
      <div class="info">
        <strong>💡 Tip:</strong> Define instant actions (timeout 0) first to prevent them 
        from retriggering after longer timeouts complete.
      </div>
    </section>
    
    <section id="example">
      <h2>Full Example Configuration</h2>
      <p>Here's the complete default configuration shipped with Stasis:</p>
      <pre><code>@author "Dustin Pilgrim"
@description "Stasis configuration file"

# Global variable
default_timeout 300  # 5 minutes

stasis:
  pre_suspend_command "hyprlock"
  monitor_media true
  ignore_remote_media true
  
  # Optional: ignore specific media players
  #media_blacklist ["spotify"]
  
  respect_idle_inhibitors true
  
  # Laptop lid behavior
  #lid_close_action "lock-screen"  # lock-screen | suspend | custom | ignore
  #lid_open_action "wake"          # wake | custom | ignore
  
  # Debounce: default is 3s; can be customized if needed
  #debounce_seconds 4
  
  # Applications that prevent idle when active
  inhibit_apps [
    "vlc"
    "Spotify"
    "mpv"
    r".*\.exe"
    r"steam_app_.*"
    r"firefox.*"
  ]
  
  # Desktop-only idle actions (applies to all devices)
  lock_screen:
    timeout 300  # 5 minutes
    command "loginctl lock-session"
    resume-command "notify-send 'Welcome Back $env.USER!'"
    lock-command "swaylock"
  end
  
  dpms:
    timeout 60  # 1 minute
    command "niri msg action power-off-monitors"
    resume-command "niri msg action power-on-monitors"
  end
  
  suspend:
    timeout 1800  # 30 minutes
    command "systemctl suspend"
    resume-command None
  end
  
  # Laptop-only AC actions
  on_ac:
    # Instant brightness adjustment
    custom-brightness-instant:
      timeout 0
      command "brightnessctl set 100%"
    end
    
    brightness:
      timeout 120  # 2 minutes
      command "brightnessctl set 30%"
    end
    
    dpms:
      timeout 60  # 1 minute
      command "niri msg action power-off-monitors"
    end
    
    lock_screen:
      timeout 120  # 2 minutes
      command "swaylock"
    end
    
    suspend:
      timeout 300  # 5 minutes
      command "systemctl suspend"
    end
  end
  
  # Laptop-only battery actions
  on_battery:
    custom-brightness-instant:
      timeout 0
      command "brightnessctl set 40%"
    end
    
    brightness:
      timeout 60  # 1 minute
      command "brightnessctl set 20%"
    end
    
    dpms:
      timeout 30  # 30 seconds
      command "niri msg action power-off-monitors"
      resume-command "niri msg action power-on-monitors"
    end
    
    lock_screen:
      timeout 120  # 2 minutes
      command "swaylock"
      resume-command "notify-send 'Welcome back $env.USER!'"
    end
    
    suspend:
      timeout 120  # 2 minutes
      command "systemctl suspend"
    end
  end
end</code></pre>
    </section>
  </main>
</div>

<style>
  .page-container {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 40px;
    max-width: 1400px;
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
    scroll-margin-top: 70px;
  }
  
  h3 {
    font-size: 1.3rem;
    font-weight: 600;
    margin: 32px 0 12px 0;
    color: var(--text-primary);
  }
  
  section {
    margin-bottom: 48px;
    scroll-margin-top: 70px;
  }
  
  p {
    line-height: 1.7;
    color: var(--text-primary);
    margin: 16px 0;
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
  
  .warning {
    background: rgba(255, 193, 7, 0.1);
    border-left: 4px solid #ffc107;
    padding: 20px;
    margin: 24px 0;
    border-radius: 4px;
  }
  
  .warning strong {
    display: block;
    margin-bottom: 12px;
    font-size: 1.05rem;
  }
  
  .warning pre {
    margin: 12px 0;
  }
  
  .warning p {
    margin: 8px 0;
  }
  
  .info {
    background: var(--bg-secondary);
    border-left: 4px solid var(--accent);
    padding: 20px;
    margin: 24px 0;
    border-radius: 4px;
  }
  
  .info strong {
    display: block;
    margin-bottom: 8px;
    color: var(--accent);
  }
  
  .info ul {
    margin: 8px 0;
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
    padding: 20px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 20px 0;
    border: 1px solid var(--border-color);
    line-height: 1.5;
  }
  
  pre code {
    background: none;
    padding: 0;
    font-size: 0.9rem;
    word-break: normal;
  }
  
  @media (max-width: 968px) {
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
      padding: 10px 8px;
      font-size: 0.75rem;
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
    
    h3 {
      font-size: 1.15rem;
      margin: 24px 0 10px 0;
    }
    
    section {
      margin-bottom: 32px;
      scroll-margin-top: 100px; /* Account for hamburger menu */
    }
    
    p {
      font-size: 0.95rem;
    }
    
    ul {
      font-size: 0.95rem;
      padding-left: 20px;
    }
    
    .warning,
    .info {
      padding: 14px;
      font-size: 0.9rem;
    }
    
    .warning strong,
    .info strong {
      font-size: 0.95rem;
    }
    
    code {
      font-size: 0.85em;
    }
    
    pre {
      padding: 12px;
      font-size: 0.75rem;
      margin: 12px 0;
    }
    
    pre code {
      font-size: 0.75rem;
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
    
    h3 {
      font-size: 1.1rem;
    }
    
    pre {
      font-size: 0.7rem;
      padding: 10px;
    }
    
    pre code {
      font-size: 0.7rem;
    }
  }
</style>
