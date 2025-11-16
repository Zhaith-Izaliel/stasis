<script lang="ts">
  import { fly, fade } from 'svelte/transition';
  import { quintOut } from 'svelte/easing';
  import { base } from '$app/paths';
  
  let { isOpen = $bindable(false) } = $props();
  
  function closeSidebar() {
    isOpen = false;
  }
  
  // Close sidebar when clicking a link
  function handleLinkClick() {
    closeSidebar();
  }
</script>

{#if isOpen}
  <!-- Overlay backdrop -->
  <button
    class="overlay"
    onclick={closeSidebar}
    transition:fade={{ duration: 300 }}
    aria-label="Close menu"
  ></button>
  
  <!-- Sidebar -->
  <nav
    class="sidebar"
    transition:fly={{ x: -300, duration: 300, easing: quintOut }}
  >
    <div class="sidebar-header">
      <h2>Menu</h2>
      <button class="close-btn" onclick={closeSidebar} aria-label="Close menu">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    </div>
    
    <div class="sidebar-content">
      <a href="{base}/" onclick={handleLinkClick}>Home</a>
      <a href="{base}/quick-start" onclick={handleLinkClick}>Quick Start</a>
      <a href="{base}/configuration" onclick={handleLinkClick}>Configuration</a>
      <a href="{base}/integration" onclick={handleLinkClick}>Integration</a>
      <a href="{base}/contributing" onclick={handleLinkClick}>Contributing</a>
      <a href="{base}/faq" onclick={handleLinkClick}>FAQ</a>
    </div>
  </nav>
{/if}

<style>
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: rgba(0, 0, 0, 0.5);
    z-index: 998;
    border: none;
    padding: 0;
    cursor: pointer;
  }
  
  .sidebar {
    position: fixed;
    top: 0;
    left: 0;
    width: 280px;
    height: 100%;
    background: var(--bg-primary);
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.3);
    z-index: 999;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  
  .sidebar-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.5rem;
    border-bottom: 1px solid var(--border-color);
  }
  
  .sidebar-header h2 {
    margin: 0;
    font-size: 1.25rem;
    font-weight: 600;
    color: var(--text-primary);
  }
  
  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all 0.2s;
    border-radius: 4px;
  }
  
  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-secondary);
  }
  
  .sidebar-content {
    display: flex;
    flex-direction: column;
    padding: 1rem;
  }
  
  .sidebar-content a {
    padding: 1rem;
    text-decoration: none;
    color: var(--text-primary);
    border-radius: 6px;
    transition: all 0.2s;
    font-size: 1rem;
    font-weight: 500;
  }
  
  .sidebar-content a:hover {
    background: var(--bg-secondary);
    color: var(--accent);
  }
  
  .sidebar-content a:active {
    background: var(--bg-secondary);
    opacity: 0.8;
  }
</style>
