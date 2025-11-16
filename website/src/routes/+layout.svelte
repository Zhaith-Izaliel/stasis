<script lang="ts">
  /* Top Level Styles */
  import '../app.css';
  /* Components */
  import TopBar from '$lib/components/TopBar.svelte';
  import SideBar from '$lib/components/SideBar.svelte';
  import Footer from '$lib/components/Footer.svelte';
  import { afterNavigate } from '$app/navigation';
  import { page } from '$app/state';
  import { base } from '$app/paths';

  let { children } = $props();
  
  let isSidebarOpen = $state(false);
  
  function toggleSidebar() {
    isSidebarOpen = !isSidebarOpen;
  }
  
  // Handle hash scrolling after navigation
  afterNavigate(({ to }) => {
    // Close sidebar on navigation
    isSidebarOpen = false;
    
    if (to?.url.hash) {
      const hash = to.url.hash.slice(1);
      const element = document.getElementById(hash);
      
      if (element) {
        // Wait for next tick to ensure DOM is ready
        setTimeout(() => {
          const topbarHeight = 70;
          const elementPosition = element.getBoundingClientRect().top + window.scrollY;
          const offsetPosition = elementPosition - topbarHeight;
          
          window.scrollTo({
            top: offsetPosition,
            behavior: 'instant'
          });
        }, 0);
      }
    }
  });
</script>

<svelte:head>
  <link rel="icon" type="image/png" href={base + '/favicon.png'} />
</svelte:head>

<div class="layout">
  <!-- Hamburger menu button (mobile only) -->
  <button class="hamburger" onclick={toggleSidebar} aria-label="Open menu">
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="3" y1="6" x2="21" y2="6"></line>
      <line x1="3" y1="12" x2="21" y2="12"></line>
      <line x1="3" y1="18" x2="21" y2="18"></line>
    </svg>
  </button>
  
  <!-- Mobile Sidebar -->
  <SideBar bind:isOpen={isSidebarOpen} />
  
  <!-- Desktop TopBar -->
  <TopBar />
  
  <div class="inner">   
    {#if page.route.id === '/'}
      {@render children()}
    {:else}
      <div class="content">
        {@render children()}
      </div>
    {/if}
  </div>
  
  <Footer />
</div>

<style>
  .layout {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 100vh;
  }
  
  .inner {
    display: grid;
    grid-template-columns: 1fr;
  }
  
  .content {
    width: 100%;
    max-width: 1200px;
    margin: 0 auto;
    padding: 2rem 1rem;
  }
  
  /* Hamburger menu button */
  .hamburger {
    position: fixed;
    top: 1rem;
    left: 1rem;
    z-index: 997;
    background: var(--bg-primary);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 0.75rem;
    cursor: pointer;
    display: none;
    align-items: center;
    justify-content: center;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
    transition: all 0.2s;
    color: var(--text-primary);
  }
  
  .hamburger:hover {
    background: var(--bg-secondary);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
  }
  
  .hamburger:active {
    transform: scale(0.95);
  }
  
  /* Tablet and Mobile styles */
  @media (max-width: 1024px) {
    .hamburger {
      display: flex;
    }
    
    /* Hide desktop TopBar on tablet and mobile */
    .layout :global(.topbar) {
      display: none;
    }
    
    .content {
      padding: 1rem;
      padding-top: 4rem; /* Space for hamburger button */
    }
  }
  
  /* Desktop styles */
  @media (min-width: 1025px) {
    .hamburger {
      display: none;
    }
    
    .content {
      padding: 2rem;
    }
  }
</style>
