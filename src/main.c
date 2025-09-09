#include <stdlib.h>
#include <string.h>
#include <ncurses.h>

int main() {
    // Initialize PDCurses
    initscr();
    
    // Enable special keys (arrow keys, function keys, etc.)
    keypad(stdscr, TRUE);
    
    // Don't echo pressed keys to screen
    noecho();
    
    // Enable colors if available
    if (has_colors()) {
        start_color();
        init_pair(1, COLOR_RED, COLOR_BLACK);
        init_pair(2, COLOR_GREEN, COLOR_BLACK);
        init_pair(3, COLOR_BLUE, COLOR_BLACK);
    }
    
    // Clear screen and display welcome message
    clear();
    
    // Get screen dimensions
    int height, width;
    getmaxyx(stdscr, height, width);
    
    // Display title
    attron(COLOR_PAIR(1) | A_BOLD);
    mvprintw(1, (width - strlen("PDCurses Demo")) / 2, "PDCurses Demo");
    attroff(COLOR_PAIR(1) | A_BOLD);
    
    // Display instructions
    attron(COLOR_PAIR(2));
    mvprintw(3, 2, "Instructions:");
    mvprintw(4, 2, "- Use arrow keys to move the cursor");
    mvprintw(5, 2, "- Press 'q' to quit");
    mvprintw(6, 2, "- Press any other key to see its code");
    attroff(COLOR_PAIR(2));
    
    // Display current position
    int cursor_y = height / 2;
    int cursor_x = width / 2;
    
    // Main loop
    int ch;
    while ((ch = getch()) != 'q') {
        // Clear the status line
        move(height - 2, 0);
        clrtoeol();
        
        // Handle different key types
        switch (ch) {
            case KEY_UP:
                if (cursor_y > 8) cursor_y--;
                mvprintw(height - 2, 2, "Key: UP ARROW");
                break;
            case KEY_DOWN:
                if (cursor_y < height - 3) cursor_y++;
                mvprintw(height - 2, 2, "Key: DOWN ARROW");
                break;
            case KEY_LEFT:
                if (cursor_x > 1) cursor_x--;
                mvprintw(height - 2, 2, "Key: LEFT ARROW");
                break;
            case KEY_RIGHT:
                if (cursor_x < width - 2) cursor_x++;
                mvprintw(height - 2, 2, "Key: RIGHT ARROW");
                break;
            default:
                if (ch >= 32 && ch <= 126) {
                    mvprintw(height - 2, 2, "Key: '%c' (ASCII: %d)", ch, ch);
                } else {
                    mvprintw(height - 2, 2, "Key code: %d", ch);
                }
                break;
        }
        
        // Update cursor position display
        attron(COLOR_PAIR(3));
        mvprintw(8, 2, "Current position: (%d, %d)", cursor_x, cursor_y);
        attroff(COLOR_PAIR(3));
        
        // Move cursor to current position and highlight it
        mvaddch(cursor_y, cursor_x, 'X' | A_BOLD | COLOR_PAIR(1));
        
        // Refresh screen
        refresh();
    }
    
    // Cleanup and exit
    endwin();
    
    return 0;
}
