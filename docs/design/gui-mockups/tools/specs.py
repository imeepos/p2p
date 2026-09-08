"""Specs for the p2p GUI (apps/gui) mockup set — grounded in real code.

Sources: apps/gui/src/config/menu.def.ts (8 rail entries),
docs/design/app-shell-redesign.md (w-14 rail, h-14 top bar with node pill,
h-8 bottom status bar), index.css (--primary #07c160, light + dark),
views/{chat,contacts,network,messages,agents,settings}.
"""

COMMON = (
    "Tauri desktop app window, light theme, white background, WeChat-green "
    "brand. Flat vector UI mockup style, crisp edges, no photo textures. "
    "All interface text in English, small crisp legible labels. "
)

RAIL = (
    "LEFT ICON RAIL 56px wide: traffic-light dots at top, then app icon, "
    "then vertical icon stack with tiny labels: Chat, Contacts, Network, "
    "Messages, Docs, LLM Share, Agents; Settings icon pinned at bottom. "
)
TOPBAR = (
    "TOP BAR: window title 'p2p', a green node status pill 'Node Online', "
    "a small Start/Stop toggle, theme and language icons on the right. "
)
STATUSBAR = (
    "BOTTOM STATUS BAR: 'Running', 'Port 47101', '12 connections', "
    "'v0.1.3' right-aligned. "
)

BOARD = (
    "UI design system board for a peer-to-peer messenger desktop app, "
    "light theme. Flat 2D presentation board on a light gray backdrop, "
    "clean labeled grid with numbered sections. Sections: "
    "(1) COLOR PALETTE - rounded swatch chips with hex labels: "
    "'BG-0 #FFFFFF', 'BG-1 #F7F8FA', 'BG-2 #EFF1F3', "
    "'PRIMARY #07C160' WeChat green, 'INFO #3B82F6', 'WARNING #F59E0B', "
    "'ERROR #EF4444', and a dark-mode row 'DARK BG #252525'. "
    "(2) TYPOGRAPHY - samples 'Display 28', 'Title 20', 'Body 14', "
    "'Caption 11' in a geometric sans-serif plus a monospace code sample. "
    "(3) BUTTONS - primary green button in four states labeled "
    "'Default', 'Hover', 'Pressed', 'Disabled'. "
    "(4) INPUT - a white text field with placeholder 'Message' and a "
    "green focus ring. "
    "(5) CHAT BUBBLES - right-aligned green user bubble 'Hey, is the node up?' "
    "and left-aligned white friend bubble 'Yes, port 47101 is listening.' "
    "(6) STATUS PILLS - 'Online' green, 'Connecting' amber, 'Offline' gray, "
    "'Error' red. "
    "(7) ICONS - row of 1.5px line icons: chat bubble, users, network, "
    "bell, book, share, sparkles, gear. "
    "(8) RADII AND SPACING - radius chips '6', '10', '16' and spacing bar "
    "'4 / 8 / 12 / 16 / 24'. " + COMMON
)

CHAT = (
    "Screenshot of the Chat page of the same desktop app. " + RAIL + TOPBAR +
    "Two-pane chat layout. LEFT CONVERSATION LIST 280px: header 'Chats' "
    "with a green '+ New' button; search field; list rows with avatar, "
    "name, snippet and time: friend 'Alice' - 'port 47101 is listening' - "
    "'2m', group 'p2p-dev (6)' - 'Alice: merge it' - '10m', agent "
    "'ops-agent' - 'task done' - '1h'. RIGHT CONVERSATION PANE: header "
    "'Alice' with green 'Online' dot; message thread: left white bubble "
    "'Hey, did you rotate the relay key?', right green bubble 'Yes, new "
    "key is live.', left white bubble 'Share the invite link?', a share "
    "link card with green icon and text 'p2p://invite/8f2k'; typing "
    "indicator. BOTTOM COMPOSER: rounded input 'Message', emoji and "
    "paperclip icons, char count '0/2000', green send button. " +
    STATUSBAR + COMMON
)

CONTACTS = (
    "Screenshot of the Contacts page of the same desktop app. " + RAIL + TOPBAR +
    "Two-pane layout. LEFT TREE 300px: header 'Contacts' with green "
    "'+ Add' button; sections 'FRIENDS (12)' with rows Alice green dot, "
    "Bob gray dot, Carol green dot; 'GROUPS (4)' with rows 'p2p-dev', "
    "'friends'; 'AGENTS (3)' with rows 'ops-agent' green dot, "
    "'docs-agent' gray dot. RIGHT DETAIL PANEL: friend profile 'Alice' "
    "with avatar, identity chip 'p2p://alice@key8f2k', a 'Capabilities' "
    "card listing 'relay', 'chat', 'file-share' as green chips, a "
    "'Danger Zone' card with a red 'Remove contact' button, and a green "
    "'Send Message' button. " + STATUSBAR + COMMON
)

NETWORK = (
    "Screenshot of the Network page of the same desktop app. " + RAIL + TOPBAR +
    "Page header 'Network' with tab bar 'Overview', 'Peers', 'Discovery', "
    "'Relay', 'Events', 'Diagnostics', the 'Peers' tab active. Content: "
    "a row of three stat cards 'Peers 12', 'Relays 3', 'Messages/s 42'; "
    "a line chart of connections over time with green line; a peers "
    "table with columns 'Peer', 'Addr', 'RTT', 'State' and rows "
    "'alice /p2p/alice... /ip4/10.0.0.3 23ms Online', 'bob 10.0.0.7 "
    "51ms Online', 'relay-eu 88ms Relay'; one row with red 'Error' "
    "state. " + STATUSBAR + COMMON
)

MESSAGES = (
    "Screenshot of the Messages center page of the same desktop app. " + RAIL + TOPBAR +
    "Page header 'Messages' with a green badge '3'. Two stacked cards. "
    "CARD 'Friend Invites': rows with avatar, name, message and two "
    "buttons: 'Dave - wants to connect' with green 'Accept' and gray "
    "'Decline'; 'Erin - wants to connect' same buttons. CARD 'Group "
    "Invites': row 'p2p-dev - Alice invited you' with green 'Accept' "
    "and gray 'Decline' buttons; one older row 'group 'lab'' with "
    "gray 'Expired' pill. " + STATUSBAR + COMMON
)

AGENTS = (
    "Screenshot of the Agents page of the same desktop app. " + RAIL + TOPBAR +
    "Page header 'Agents' with a green 'Create Agent' button. Three "
    "sections. SECTION 'Mine (2)': agent cards with sparkles icon, "
    "name, skills chips and online dot: 'ops-agent' chips 'deploy', "
    "'monitor' green dot; 'docs-agent' chip 'write' gray dot. SECTION "
    "'Discover': two agent cards from the network with 'Invite' "
    "buttons: 'translate-agent' chips 'i18n', 'review'; "
    "'test-agent' chip 'pytest'. SECTION 'Invites': one row "
    "'translate-agent invited you' with green 'Accept' button. " +
    STATUSBAR + COMMON
)

SETTINGS = (
    "Screenshot of the Settings page of the same desktop app. " + RAIL + TOPBAR +
    "Settings list layout with sections. SECTION 'General': rows "
    "'Theme' with segmented control 'Light' selected green, 'Dark', "
    "'System'; 'Language' with value 'English'; 'Launch at login' with "
    "green toggle on. SECTION 'Node': rows 'Listen port' with "
    "monospace value '47101', 'Bootstrap peers' with value '4', "
    "'Relay mode' with dropdown 'Auto'. SECTION 'About': rows "
    "'Version' with monospace 'v0.1.3', 'Check updates' with a gray "
    "'Up to date' pill. " + STATUSBAR + COMMON
)

SPECS = [
    {"id": "01-design-system", "file": "01-design-system.png", "mode": "generations", "prompt": BOARD},
    {"id": "02-chat",          "file": "02-chat.png",          "mode": "edits",       "prompt": CHAT},
    {"id": "03-network",       "file": "03-network.png",       "mode": "edits",       "prompt": NETWORK},
    {"id": "04-contacts",      "file": "04-contacts.png",      "mode": "edits",       "prompt": CONTACTS},
    {"id": "05-messages",      "file": "05-messages.png",      "mode": "edits",       "prompt": MESSAGES},
    {"id": "06-agents",        "file": "06-agents.png",        "mode": "edits",       "prompt": AGENTS},
    {"id": "07-settings",      "file": "07-settings.png",      "mode": "edits",       "prompt": SETTINGS},
]
