#!/usr/bin/env node
/**
 * Playwright Bridge - Executes browser commands and returns results
 *
 * Usage: node playwright_bridge.js <command> [args...]
 *
 * Commands:
 *   start                    - Launch browser
 *   goto <url>               - Navigate to URL
 *   search_ebay <query>      - Search eBay
 *   search_google <query>    - Search Google
 *   click <selector>         - Click element
 *   type <selector> <text>   - Type into input
 *   scroll <direction>       - Scroll up/down
 *   get_text <selector>      - Get text content
 *   get_links                - Get all links on page
 *   screenshot <path>        - Take screenshot
 *   get_state                - Get current URL, title, etc.
 *   close                    - Close browser
 */

const { chromium } = require('playwright');

// Launch browser with CDP enabled so we can reconnect
async function getPage() {
    // Try to connect to existing Chromium via CDP port 9222
    try {
        const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
        const contexts = browser.contexts();
        if (contexts.length > 0 && contexts[0].pages().length > 0) {
            return { browser, page: contexts[0].pages()[0], isNew: false, keepOpen: true };
        }
    } catch (e) {
        // No existing browser with CDP, launch new one
    }

    // Launch with remote debugging so we can reconnect later
    const browser = await chromium.launch({
        headless: false,
        args: [
            '--start-maximized',
            '--remote-debugging-port=9222'
        ]
    });
    const context = await browser.newContext({ viewport: { width: 1920, height: 1080 } });
    const page = await context.newPage();
    return { browser, page, isNew: true, keepOpen: true };
}

async function main() {
    const [,, command, ...args] = process.argv;

    if (!command) {
        console.log(JSON.stringify({ error: 'No command specified' }));
        process.exit(1);
    }

    try {
        const { browser, page, isNew } = await getPage();
        let result = {};

        switch (command) {
            case 'start':
                result = { success: true, message: isNew ? 'Browser launched' : 'Browser already running' };
                break;

            case 'goto':
                const url = args[0];
                await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 30000 });
                result = { success: true, url: page.url(), title: await page.title() };
                break;

            case 'search_ebay':
                const ebayQuery = args.join(' ');
                const ebayUrl = `https://www.ebay.com/sch/i.html?_nkw=${encodeURIComponent(ebayQuery)}`;
                await page.goto(ebayUrl, { waitUntil: 'domcontentloaded' });
                await page.waitForTimeout(1000);
                result = {
                    success: true,
                    url: page.url(),
                    title: await page.title(),
                    query: ebayQuery
                };
                break;

            case 'search_google':
                const googleQuery = args.join(' ');
                const googleUrl = `https://www.google.com/search?q=${encodeURIComponent(googleQuery)}`;
                await page.goto(googleUrl, { waitUntil: 'domcontentloaded' });
                result = { success: true, url: page.url(), query: googleQuery };
                break;

            case 'click':
                const selector = args[0];
                await page.click(selector, { timeout: 5000 });
                result = { success: true, clicked: selector };
                break;

            case 'type':
                const typeSelector = args[0];
                const text = args.slice(1).join(' ');
                await page.fill(typeSelector, text);
                result = { success: true, typed: text };
                break;

            case 'scroll':
                const direction = args[0] || 'down';
                const delta = direction === 'up' ? -500 : 500;
                await page.evaluate((d) => window.scrollBy(0, d), delta);
                result = { success: true, scrolled: direction };
                break;

            case 'get_text':
                const textSelector = args[0];
                const textContent = await page.textContent(textSelector);
                result = { success: true, text: textContent };
                break;

            case 'get_links':
                const links = await page.evaluate(() => {
                    return Array.from(document.querySelectorAll('a[href]'))
                        .slice(0, 30)
                        .map(a => ({
                            text: a.innerText.trim().slice(0, 100),
                            href: a.href
                        }))
                        .filter(l => l.text && l.href.startsWith('http'));
                });
                result = { success: true, links };
                break;

            case 'get_items':
                // eBay specific - get product listings
                const items = await page.evaluate(() => {
                    return Array.from(document.querySelectorAll('.s-item'))
                        .slice(0, 20)
                        .map(item => ({
                            title: item.querySelector('.s-item__title')?.innerText || '',
                            price: item.querySelector('.s-item__price')?.innerText || '',
                            link: item.querySelector('a.s-item__link')?.href || '',
                            shipping: item.querySelector('.s-item__shipping')?.innerText || ''
                        }))
                        .filter(i => i.title && i.price);
                });
                result = { success: true, items };
                break;

            case 'get_markdown':
                // Convert page content to clean markdown (Markdowser-style)
                // Strips ads, nav, footer - keeps main content
                const markdown = await page.evaluate(() => {
                    // Remove noise elements before extraction
                    const noiseSelectors = [
                        'nav', 'header', 'footer', 'aside',
                        '[class*="ad"]', '[class*="banner"]', '[class*="popup"]',
                        '[class*="cookie"]', '[class*="newsletter"]', '[class*="social"]',
                        '[id*="ad"]', '[id*="banner"]', '[role="navigation"]',
                        'script', 'style', 'noscript', 'iframe'
                    ];

                    // Clone document to not modify original
                    const clone = document.body.cloneNode(true);

                    // Remove noise
                    noiseSelectors.forEach(sel => {
                        clone.querySelectorAll(sel).forEach(el => el.remove());
                    });

                    // Convert to markdown-ish format
                    function toMarkdown(node, depth = 0) {
                        if (node.nodeType === Node.TEXT_NODE) {
                            return node.textContent.trim();
                        }
                        if (node.nodeType !== Node.ELEMENT_NODE) return '';

                        const tag = node.tagName.toLowerCase();
                        const children = Array.from(node.childNodes)
                            .map(c => toMarkdown(c, depth + 1))
                            .filter(t => t)
                            .join(' ');

                        if (!children) return '';

                        switch(tag) {
                            case 'h1': return `\n# ${children}\n`;
                            case 'h2': return `\n## ${children}\n`;
                            case 'h3': return `\n### ${children}\n`;
                            case 'h4': return `\n#### ${children}\n`;
                            case 'p': return `\n${children}\n`;
                            case 'li': return `- ${children}\n`;
                            case 'a': {
                                const href = node.getAttribute('href');
                                return href ? `[${children}](${href})` : children;
                            }
                            case 'strong':
                            case 'b': return `**${children}**`;
                            case 'em':
                            case 'i': return `*${children}*`;
                            case 'code': return `\`${children}\``;
                            case 'pre': return `\n\`\`\`\n${children}\n\`\`\`\n`;
                            case 'blockquote': return `\n> ${children}\n`;
                            case 'br': return '\n';
                            case 'hr': return '\n---\n';
                            case 'img': {
                                const alt = node.getAttribute('alt') || 'image';
                                const src = node.getAttribute('src');
                                return src ? `![${alt}](${src})` : '';
                            }
                            default: return children;
                        }
                    }

                    // Find main content area (try common patterns)
                    const mainContent = clone.querySelector('main, article, [role="main"], .content, #content, .main')
                        || clone;

                    let md = toMarkdown(mainContent);

                    // Clean up excessive whitespace
                    md = md.replace(/\n{3,}/g, '\n\n').trim();

                    // Truncate if too long
                    if (md.length > 8000) {
                        md = md.slice(0, 8000) + '\n\n[... truncated ...]';
                    }

                    return md;
                });

                result = {
                    success: true,
                    markdown,
                    url: page.url(),
                    title: await page.title(),
                    length: markdown.length
                };
                break;

            case 'get_structured':
                // Get structured data about the page (for planner)
                const structured = await page.evaluate(() => {
                    const data = {
                        title: document.title,
                        headings: [],
                        links: [],
                        forms: [],
                        buttons: [],
                        inputs: [],
                        images: []
                    };

                    // Headings
                    document.querySelectorAll('h1, h2, h3').forEach(h => {
                        data.headings.push({
                            level: parseInt(h.tagName[1]),
                            text: h.innerText.slice(0, 100)
                        });
                    });

                    // Links (limit to visible, meaningful ones)
                    document.querySelectorAll('a[href]').forEach(a => {
                        if (a.offsetParent && a.innerText.trim()) {
                            data.links.push({
                                text: a.innerText.slice(0, 50),
                                href: a.href
                            });
                        }
                    });
                    data.links = data.links.slice(0, 20);

                    // Forms
                    document.querySelectorAll('form').forEach(f => {
                        data.forms.push({
                            action: f.action,
                            method: f.method,
                            inputs: Array.from(f.querySelectorAll('input')).map(i => ({
                                name: i.name,
                                type: i.type,
                                placeholder: i.placeholder
                            }))
                        });
                    });

                    // Buttons
                    document.querySelectorAll('button, input[type="submit"], [role="button"]').forEach(b => {
                        if (b.offsetParent) {
                            data.buttons.push({
                                text: b.innerText?.slice(0, 50) || b.value || b.getAttribute('aria-label'),
                                type: b.type
                            });
                        }
                    });
                    data.buttons = data.buttons.slice(0, 15);

                    // Inputs (not in forms)
                    document.querySelectorAll('input:not(form input), textarea').forEach(i => {
                        if (i.offsetParent) {
                            data.inputs.push({
                                type: i.type,
                                name: i.name,
                                placeholder: i.placeholder,
                                id: i.id
                            });
                        }
                    });

                    return data;
                });

                result = { success: true, ...structured };
                break;

            case 'screenshot':
                const screenshotPath = args[0] || '/tmp/screenshot.png';
                await page.screenshot({ path: screenshotPath, fullPage: false });
                result = { success: true, path: screenshotPath };
                break;

            case 'get_state':
                result = {
                    success: true,
                    url: page.url(),
                    title: await page.title(),
                    viewport: page.viewportSize()
                };
                break;

            case 'detect_obstacles':
                // Detect common obstacles: cookie banners, popups, modals
                const obstacles = [];

                // Check for cookie consent
                const cookieIndicators = [
                    '[class*="cookie"]', '[id*="cookie"]',
                    '[class*="consent"]', '[id*="consent"]',
                    '[class*="gdpr"]', '[id*="gdpr"]',
                    '[class*="privacy"]',
                ];
                for (const sel of cookieIndicators) {
                    const el = await page.$(sel);
                    if (el && await el.isVisible()) {
                        obstacles.push({ type: 'cookie_consent', selector: sel });
                        break;
                    }
                }

                // Check for modal/popup overlays
                const modalIndicators = [
                    '[class*="modal"]:visible',
                    '[class*="popup"]:visible',
                    '[class*="overlay"]:visible',
                    '[role="dialog"]',
                ];
                for (const sel of modalIndicators) {
                    try {
                        const el = await page.$(sel);
                        if (el && await el.isVisible()) {
                            const text = await el.innerText();
                            if (!text.toLowerCase().includes('cookie')) {
                                obstacles.push({ type: 'modal', selector: sel });
                            }
                        }
                    } catch(e) {}
                }

                result = { success: true, obstacles };
                break;

            case 'dismiss_cookies':
                // GDPR Cookie Consent Removal - Reject all non-essential
                // Try multiple strategies, sites use different patterns
                const cookieSelectors = [
                    // Reject/Decline buttons (preferred)
                    'button:has-text("Reject all")',
                    'button:has-text("Reject All")',
                    'button:has-text("Decline all")',
                    'button:has-text("Decline All")',
                    'button:has-text("Only essential")',
                    'button:has-text("Only necessary")',
                    'button:has-text("Refuse")',
                    'button:has-text("Deny")',
                    '[data-testid="reject-all"]',
                    '#reject-all',
                    '.reject-all',

                    // "Manage" then reject pattern
                    'button:has-text("Manage")',
                    'button:has-text("Customize")',
                    'button:has-text("Settings")',

                    // Last resort - any close/dismiss
                    'button:has-text("Close")',
                    'button:has-text("×")',
                    '[aria-label="Close"]',
                    '.cookie-banner-close',
                    '#cookie-close',
                ];

                let dismissed = false;
                for (const selector of cookieSelectors) {
                    try {
                        const el = await page.$(selector);
                        if (el && await el.isVisible()) {
                            await el.click();
                            dismissed = true;
                            await page.waitForTimeout(500);

                            // If we clicked "Manage", look for reject in the new dialog
                            if (selector.includes('Manage') || selector.includes('Customize')) {
                                const rejectInDialog = await page.$('button:has-text("Reject"), button:has-text("Save"), button:has-text("Confirm")');
                                if (rejectInDialog) {
                                    // Uncheck all optional checkboxes first
                                    const checkboxes = await page.$$('input[type="checkbox"]:checked');
                                    for (const cb of checkboxes) {
                                        const label = await cb.evaluate(el => el.closest('label')?.innerText || '');
                                        if (!label.toLowerCase().includes('necessary') &&
                                            !label.toLowerCase().includes('essential') &&
                                            !label.toLowerCase().includes('required')) {
                                            await cb.click();
                                        }
                                    }
                                    await rejectInDialog.click();
                                }
                            }
                            break;
                        }
                    } catch (e) {
                        // Selector didn't match, try next
                    }
                }

                result = { success: dismissed, message: dismissed ? 'Cookie consent handled' : 'No cookie dialog found' };
                break;

            case 'close':
                await browser.close();
                try { fs.unlinkSync(STATE_FILE); } catch(e) {}
                result = { success: true, message: 'Browser closed' };
                break;

            default:
                result = { error: `Unknown command: ${command}` };
        }

        console.log(JSON.stringify(result));

        // Disconnect without closing browser (for CDP connections)
        if (command !== 'close') {
            // Just disconnect, don't close
            browser.close = () => {}; // Prevent auto-close
        }

    } catch (e) {
        console.log(JSON.stringify({ error: e.message }));
        process.exit(1);
    }
}

main().then(() => process.exit(0));
