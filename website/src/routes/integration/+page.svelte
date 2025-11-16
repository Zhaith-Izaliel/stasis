<script lang="ts">
  import { onMount } from 'svelte';
  
  let activeSection = $state('');
  
  const sections = [
    { id: 'waybar', title: 'Waybar' }
  ];
  
  onMount(() => {
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
      const topbarHeight = 70;
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
    <h1>Integration</h1>
    
    <section id="waybar">
      <h2>Waybar</h2>
      
      <h3>Example Custom Module</h3>
      <p>To use Stasis with waybar is fairly straightforward. Below is an example custom module for waybar:</p>
      
      <h4>Icon-based Display</h4>
      <pre><code>"custom/stasis": {'{'}
  "exec": "stasis info --json",
  "format": "{'{'}icon{'}'}",
  "format-icons": {'{'}
      "idle_active": "",
      "idle_inhibited": "",
      "manually_inhibited": "",
      "not_running": "󰒲"
  {'}'},
  "tooltip": true,
  "on-click": "stasis toggle-inhibit",
  "interval": 2,
  "restart-interval": 2,
  "return-type": "json"
{'}'}</code></pre>

      <h4>Text-based Display</h4>
      <p>Or you can just display text if you don't want icons:</p>
      <pre><code>"custom/stasis": {'{'}
  "exec": "stasis info --json",
  "format": "{'{'}text{'}'}",
  "tooltip": true,
  "on-click": "stasis toggle-inhibit",
  "interval": 2,
  "restart-interval": 2,
  "return-type": "json"
{'}'}</code></pre>
    </section>
  </main>
</div>

<style>
/* === LAYOUT === */
.page-container {
  display: grid;
  grid-template-columns: 200px 1fr;
  gap: 40px;
  max-width: 1200px;
  margin: 0 auto;
  padding: 40px 20px;
}

/* === SIDE NAV === */
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
  scroll-margin-top: 120px;
}

h3 {
  font-size: 1.4rem;
  font-weight: 600;
  margin: 32px 0 12px 0;
  color: var(--text-primary);
}

h4 {
  font-size: 1.1rem;
  font-weight: 600;
  margin: 24px 0 12px 0;
  color: var(--text-primary);
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

code {
  background: var(--bg-secondary);
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
  font-size: 0.9em;
  color: var(--text-primary);
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

/* === MOBILE === */
@media (max-width: 768px) {
  .page-container {
    grid-template-columns: 1fr;
    gap: 20px;
    padding: 80px 16px 20px; /* Account for fixed topbar */
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
    background: rgba(168, 85, 247, 0.1);
  }

  h1 {
    font-size: 2rem;
    margin-bottom: 24px;
  }

  h2 {
    font-size: 1.4rem;
    margin: 32px 0 12px 0;
    scroll-margin-top: 100px;
  }

  h3 {
    font-size: 1.3rem;
  }

  h4 {
    font-size: 1.05rem;
  }

  pre {
    padding: 12px;
    font-size: 0.85rem;
    margin: 12px 0;
  }
}

/* === TINY DEVICES === */
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
