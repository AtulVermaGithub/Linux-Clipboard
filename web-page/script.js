// ==========================================================================
// Javascript Controller for Linux Clipboard History Landing Page
// ==========================================================================

document.addEventListener('DOMContentLoaded', () => {
    
    // 1. Dark / Light Theme Sync
    const themeToggleBtn = document.getElementById('theme-toggle');
    const lightIcon = document.getElementById('theme-icon-light');
    const darkIcon = document.getElementById('theme-icon-dark');
    const rootHtml = document.documentElement;

    // Load initial theme from LocalStorage or system preference
    const savedTheme = localStorage.getItem('theme');
    if (savedTheme) {
        setTheme(savedTheme);
    } else {
        const prefersLight = window.matchMedia('(prefers-color-scheme: light)').matches;
        setTheme(prefersLight ? 'light' : 'dark');
    }

    themeToggleBtn.addEventListener('click', () => {
        const currentTheme = rootHtml.getAttribute('data-theme');
        const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
        setTheme(newTheme);
    });

    function setTheme(theme) {
        rootHtml.setAttribute('data-theme', theme);
        localStorage.setItem('theme', theme);
        
        const clipHistoryImg = document.getElementById('clip-history-img');
        const emojiImg = document.getElementById('emoji-img');
        
        if (theme === 'light') {
            lightIcon.classList.add('hide');
            darkIcon.classList.remove('hide');
            if (clipHistoryImg) clipHistoryImg.src = 'assets/clip_history_light.png';
            if (emojiImg) emojiImg.src = 'assets/emoji_light.png';
        } else {
            darkIcon.classList.add('hide');
            lightIcon.classList.remove('hide');
            if (clipHistoryImg) clipHistoryImg.src = 'assets/clip_history_dark.png';
            if (emojiImg) emojiImg.src = 'assets/emoji_dark.png';
        }
    }

    // 2. One-click Installation Copier
    const copyBtn = document.getElementById('copy-btn');
    const copyText = document.getElementById('copy-text');
    const installCommand = document.getElementById('install-command').innerText;

    copyBtn.addEventListener('click', async () => {
        try {
            await navigator.clipboard.writeText(installCommand);
            
            // Set Success state
            copyBtn.classList.add('success');
            copyText.textContent = 'Copied!';
            
            // Reset after 2 seconds
            setTimeout(() => {
                copyBtn.classList.remove('success');
                copyText.textContent = 'Copy';
            }, 2000);
        } catch (err) {
            console.error('Failed to copy text: ', err);
        }
    });

    // 3. Screenshot Showcase Carousel
    const tabs = document.querySelectorAll('.carousel-tab');
    const slides = document.querySelectorAll('.slide');

    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            // Get index target
            const targetIdx = parseInt(tab.getAttribute('data-target'), 10);
            
            // Remove active classes
            tabs.forEach(t => t.classList.remove('active'));
            slides.forEach(s => s.classList.remove('active'));
            
            // Apply active class to selected components
            tab.classList.add('active');
            slides[targetIdx].classList.add('active');
        });
    });

});
