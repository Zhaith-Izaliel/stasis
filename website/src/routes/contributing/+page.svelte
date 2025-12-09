<script lang="ts">
  import { onMount } from 'svelte';
  import CodeBlock from '$lib/components/CodeBlock.svelte';
  
  let activeSection = $state('');
  
  const sections = [
    { id: 'welcome', title: 'Welcome' },
    { id: 'ways-to-help', title: 'Ways to Help' },
    { id: 'getting-started', title: 'Getting Started' }
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
  
  // Code examples
  const cloneCode = `git clone https://github.com/YOUR_USERNAME/stasis.git
cd stasis`;

  const branchCode = `git checkout -b feature/your-feature-name`;
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
    <h1>Contributing</h1>
    
    <section id="welcome">
      <div class="welcome-card">
 
<p class="welcome-text">
  Contributions make Stasis better for everyone! 💜 <br /> <br />
  Please read our <a href="https://github.com/saltnpepper97/stasis/blob/main/CONTRIBUTING.md" target="_blank">GitHub Contributing Guidelines</a> before submitting anything.<br /><br />

  <strong>Important:</strong><br />
  - All new features, enhancements, or other contributions (except packaging or compositor work) must first go through an issue or bug report.<br />  
  - Packaging for distributions and compositor integration can be submitted directly.<br />
  - Using the correct labels (`bug`, `feature request`, `packaging`, `compositor`, etc.) helps maintainers manage contributions efficiently.<br />
</p>

      </div>
    </section>
    
    <section id="ways-to-help">
      <h2>Ways to Help</h2>
      <p>There are many ways to contribute to Stasis, no matter your skill level:</p>
      
      <div class="contribution-grid">
        <div class="contribution-card">
          <div class="card-icon">🐛</div>
          <h3>Report Bugs</h3>
          <p>Found something that's not working right? Open an issue on GitHub with:</p>
          <ul>
            <li>Your distro and compositor</li>
            <li>Steps to reproduce the problem</li>
            <li>Expected vs actual behavior</li>
            <li>Relevant logs from <code>~/.cache/stasis/stasis.log</code></li>
          </ul>
          <a href="https://github.com/saltnpepper97/stasis/issues/new" target="_blank" rel="noopener noreferrer" class="card-link">Report an Issue →</a>
        </div>

        <div class="contribution-card">
          <div class="card-icon">💡</div>
          <h3>Suggest Features</h3>
          <p>Have an idea for a new feature or improvement? We'd love to hear it! Share:</p>
          <ul>
            <li>Your use case and why it matters</li>
            <li>How you envision it working</li>
            <li>Any examples from other tools</li>
          </ul>
          <a href="https://github.com/saltnpepper97/stasis/issues/new" target="_blank" rel="noopener noreferrer" class="card-link">Suggest a Feature →</a>
        </div>

        <div class="contribution-card">
          <div class="card-icon">🔧</div>
          <h3>Submit Pull Requests</h3>
          <p>Ready to dive into the code? Contributions welcome for:</p>
          <ul>
            <li>Bug fixes and improvements</li>
            <li>New features (discuss first!)</li>
            <li>Code cleanup and optimization</li>
            <li>Test coverage</li>
          </ul>
          <a href="https://github.com/saltnpepper97/stasis/pulls" target="_blank" rel="noopener noreferrer" class="card-link">View Pull Requests →</a>
        </div>

        <div class="contribution-card">
          <div class="card-icon">📦</div>
          <h3>Package for Distros</h3>
          <p>Help make Stasis available to more users by:</p>
          <ul>
            <li>Creating packages for different distros</li>
            <li>Maintaining existing packages</li>
            <li>Testing on various systems</li>
          </ul>
        </div>

        <div class="contribution-card">
          <div class="card-icon">📖</div>
          <h3>Improve Documentation</h3>
          <p>Documentation is always evolving! Help by:</p>
          <ul>
            <li>Fixing typos and unclear explanations</li>
            <li>Adding examples and use cases</li>
            <li>Improving configuration guides</li>
            <li>Translating docs</li>
          </ul>
        </div>

        <div class="contribution-card">
          <div class="card-icon">🖥️</div>
          <h3>Add Compositor Support</h3>
          <p>Expand Wayland ecosystem compatibility by:</p>
          <ul>
            <li>Testing on different compositors</li>
            <li>Implementing support for new ones</li>
            <li>Documenting quirks and workarounds</li>
          </ul>
        </div>
      </div>
    </section>
    
    <section id="getting-started">
      <h2>Getting Started</h2>
      <p>Ready to contribute? Here's how to get started:</p>
      
      <div class="steps">
        <div class="step">
          <div class="step-number">1</div>
          <div class="step-content">
            <h4>Fork the Repository</h4>
            <p>Head over to the <a href="https://github.com/saltnpepper97/stasis" target="_blank" rel="noopener noreferrer">Stasis GitHub repo</a> and fork it to your account.</p>
          </div>
        </div>

        <div class="step">
          <div class="step-number">2</div>
          <div class="step-content">
            <h4>Clone Your Fork</h4>
            <CodeBlock code={cloneCode} language="bash" />
          </div>
        </div>

        <div class="step">
          <div class="step-number">3</div>
          <div class="step-content">
            <h4>Create a Branch</h4>
            <CodeBlock code={branchCode} language="bash" />
          </div>
        </div>

        <div class="step">
          <div class="step-number">4</div>
          <div class="step-content">
            <h4>Make Your Changes</h4>
            <p>Write clear, well-commented code. Test your changes thoroughly before submitting.</p>
          </div>
        </div>

        <div class="step">
          <div class="step-number">5</div>
          <div class="step-content">
            <h4>Submit a Pull Request</h4>
            <p>Push your branch and open a PR on GitHub. Describe what you've changed and why.</p>
          </div>
        </div>
      </div>
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
    margin: 0 0 12px 0;
    color: var(--text-primary);
  }

  h4 {
    font-size: 1.1rem;
    font-weight: 600;
    margin: 0 0 8px 0;
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

  .welcome-card {
    background: linear-gradient(135deg, rgba(168, 85, 247, 0.1) 0%, rgba(201, 42, 42, 0.1) 100%);
    border: 2px solid var(--accent);
    border-radius: 12px;
    padding: 32px;
    margin: 24px 0;
  }

  .welcome-text {
    font-size: 1.15rem;
    line-height: 1.8;
    margin: 0;
    color: var(--text-primary);
  }

  .contribution-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(320px, 1fr));
    gap: 24px;
    margin: 32px 0;
  }

  .contribution-card {
    background: var(--bg-secondary);
    border: 1px solid var(--border-color);
    border-radius: 12px;
    padding: 28px;
    transition: all 0.3s ease;
  }

  .contribution-card:hover {
    transform: translateY(-4px);
    border-color: var(--accent);
    box-shadow: 0 8px 24px rgba(168, 85, 247, 0.15);
  }

  .card-icon {
    font-size: 2.5rem;
    margin-bottom: 16px;
  }

  .contribution-card ul {
    margin: 16px 0;
    padding-left: 20px;
    line-height: 1.8;
  }

  .contribution-card li {
    margin: 8px 0;
    color: var(--text-secondary);
  }

  .card-link {
    display: inline-block;
    margin-top: 16px;
    color: var(--accent);
    text-decoration: none;
    font-weight: 600;
    transition: opacity 0.2s ease;
  }

  .card-link:hover {
    opacity: 0.8;
  }

  .steps {
    margin: 32px 0;
  }

  .step {
    display: flex;
    gap: 24px;
    margin-bottom: 32px;
  }

  .step-number {
    flex-shrink: 0;
    width: 48px;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: linear-gradient(135deg, #a855f7 0%, #c92a2a 100%);
    color: white;
    font-weight: 700;
    font-size: 1.3rem;
    border-radius: 50%;
  }

  .step-content {
    flex: 1;
  }

  .step-content h4 {
    margin-top: 8px;
  }

  .info {
    background: var(--bg-secondary);
    border-left: 4px solid var(--accent);
    padding: 24px;
    margin: 32px 0;
    border-radius: 4px;
  }

  .info strong {
    display: block;
    margin-bottom: 8px;
    color: var(--accent);
    font-size: 1.05rem;
  }

  .info p {
    margin: 0;
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

  code {
    background: var(--bg-secondary);
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'Consolas', 'Monaco', 'Courier New', monospace;
    font-size: 0.9em;
    color: var(--text-primary);
  }

  @media (max-width: 968px) {
    .page-container {
      grid-template-columns: 1fr;
      gap: 20px;
      padding: 80px 16px 20px;
    }
    
    .links-nav {
      position: static;
      border-bottom: 1px solid var(--border-color);
      padding-bottom: 16px;
      margin-bottom: 8px;
    }
    
    .nav-title {
      font-size: 0.8rem;
      margin-bottom: 8px;
    }
    
    .links-nav ul {
      display: flex;
      flex-wrap: wrap;
      gap: 8px;
    }
    
    .links-nav button {
      border-left: none;
      border-bottom: 2px solid transparent;
      padding: 10px 16px;
      font-size: 0.85rem;
      background: var(--bg-secondary);
      border-radius: 6px;
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
      scroll-margin-top: 100px;
    }

    h3 {
      font-size: 1.15rem;
    }

    h4 {
      font-size: 1rem;
    }

    section {
      margin-bottom: 32px;
      scroll-margin-top: 100px;
    }

    p {
      font-size: 0.95rem;
    }

    .welcome-card {
      padding: 20px;
    }

    .welcome-text {
      font-size: 1rem;
    }

    .contribution-grid {
      grid-template-columns: 1fr;
      gap: 20px;
    }

    .contribution-card {
      padding: 20px;
    }

    .card-icon {
      font-size: 2rem;
    }

    .contribution-card ul {
      font-size: 0.9rem;
      padding-left: 18px;
    }

    .step {
      gap: 16px;
      margin-bottom: 24px;
    }

    .step-number {
      width: 40px;
      height: 40px;
      font-size: 1.1rem;
    }

    .step-content p {
      font-size: 0.9rem;
    }

    .info {
      padding: 16px;
      font-size: 0.9rem;
    }

    .info strong {
      font-size: 1rem;
    }

    code {
      font-size: 0.85em;
    }
  }

  @media (max-width: 480px) {
    .page-container {
      padding: 70px 12px 20px;
    }

    h1 {
      font-size: 1.75rem;
    }

    h2 {
      font-size: 1.25rem;
    }

    .welcome-card {
      padding: 16px;
    }

    .welcome-text {
      font-size: 0.95rem;
    }

    .contribution-card {
      padding: 16px;
    }

    .step-number {
      width: 36px;
      height: 36px;
      font-size: 1rem;
    }
  }
</style>
