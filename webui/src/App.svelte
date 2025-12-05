<script>
  import { onMount } from 'svelte';
  import { store } from './lib/store.svelte';
  import NavBar from './components/NavBar.svelte';
  import Toast from './components/Toast.svelte';
  import StatusTab from './routes/StatusTab.svelte';
  import ConfigTab from './routes/ConfigTab.svelte';
  import ModulesTab from './routes/ModulesTab.svelte';
  import LogsTab from './routes/LogsTab.svelte';
  import InfoTab from './routes/InfoTab.svelte';
  import './app.css';
  import './layout.css';
  
  let activeTab = $state('status');
  let dragOffset = $state(0);
  let isDragging = $state(false);
  let containerWidth = $state(0);

  let touchStartX = 0;
  let touchStartY = 0;

  const TABS = ['status', 'config', 'modules', 'logs', 'info'];

  function switchTab(id) {
    activeTab = id;
  }

  function handleTouchStart(e) {
    touchStartX = e.changedTouches[0].screenX;
    touchStartY = e.changedTouches[0].screenY;
    isDragging = true;
    dragOffset = 0;
  }

  function handleTouchMove(e) {
    if (!isDragging) return;

    const currentX = e.changedTouches[0].screenX;
    const currentY = e.changedTouches[0].screenY;
    let diffX = currentX - touchStartX;
    const diffY = currentY - touchStartY;

    if (Math.abs(diffY) > Math.abs(diffX)) {
      return;
    }

    if (e.cancelable) e.preventDefault();

    const currentIndex = TABS.indexOf(activeTab);

    if ((currentIndex === 0 && diffX > 0) || (currentIndex === TABS.length - 1 && diffX < 0)) {
      diffX = diffX / 3;
    }

    dragOffset = diffX;
  }

  function handleTouchEnd() {
    if (!isDragging) return;
    isDragging = false;

    const threshold = containerWidth * 0.25 || 60;
    const currentIndex = TABS.indexOf(activeTab);
    let nextIndex = currentIndex;

    if (dragOffset < -threshold && currentIndex < TABS.length - 1) {
      nextIndex = currentIndex + 1;
    } else if (dragOffset > threshold && currentIndex > 0) {
      nextIndex = currentIndex - 1;
    }

    if (nextIndex !== currentIndex) {
      switchTab(TABS[nextIndex]);
    }

    dragOffset = 0;
  }

  onMount(() => {
    store.init();
  });

  let baseTranslateX = $derived(TABS.indexOf(activeTab) * -20);
</script>

<div class="app-root">
  <NavBar {activeTab} onTabChange={switchTab} />

  <main class="main-content" 
        bind:clientWidth={containerWidth}
        ontouchstart={handleTouchStart} 
        ontouchmove={handleTouchMove}
        ontouchend={handleTouchEnd}
        ontouchcancel={handleTouchEnd}>
    
    <div class="swipe-track"
         style:transform={`translateX(calc(${baseTranslateX}% + ${dragOffset}px))`}
         style:transition={isDragging ? 'none' : 'transform 0.3s cubic-bezier(0.25, 0.8, 0.5, 1)'}>
      
      <div class="swipe-page"><div class="tab-pane"><StatusTab /></div></div>
      <div class="swipe-page"><div class="tab-pane"><ConfigTab /></div></div>
      <div class="swipe-page"><div class="tab-pane"><ModulesTab /></div></div>
      <div class="swipe-page"><div class="tab-pane"><LogsTab /></div></div>
      <div class="swipe-page"><div class="tab-pane"><InfoTab /></div></div>

    </div>
  </main>

  <Toast />
</div>

<style>
  :global(.main-content) {
    overflow-x: hidden;
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .swipe-track {
    display: flex;
    width: 500%; 
    height: 100%;
    will-change: transform;
  }

  .swipe-page {
    width: 20%;
    height: 100%;
    flex-shrink: 0;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
  }
  :global(.tab-pane) {
    height: 100%;
    box-sizing: border-box;
  }
</style>