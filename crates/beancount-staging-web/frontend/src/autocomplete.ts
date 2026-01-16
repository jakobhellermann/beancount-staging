export type FilterFunction = (query: string, items: string[]) => string[];

export class Autocomplete {
  private dropdown: HTMLDivElement;
  private visible = false;
  private selectedIndex = -1;
  private items: string[] = [];
  private currentInput: HTMLSpanElement | null = null;
  private availableItems: string[] = [];
  private onSelect: (item: string) => void;
  private filterFn: FilterFunction;

  constructor(
    availableItems: string[],
    onSelect: (item: string) => void,
    filterFn: FilterFunction,
  ) {
    this.availableItems = availableItems;
    this.onSelect = onSelect;
    this.filterFn = filterFn;

    // Create autocomplete dropdown
    this.dropdown = document.createElement("div");
    this.dropdown.className = "autocomplete-dropdown";
    this.dropdown.style.display = "none";
    document.body.appendChild(this.dropdown);

    // Close dropdown when clicking outside
    document.addEventListener("click", (e) => {
      if (!this.dropdown.contains(e.target as Node) && e.target !== this.currentInput) {
        this.hide();
      }
    });
  }

  setAvailableItems(items: string[]) {
    this.availableItems = items;
  }

  show(inputEl: HTMLSpanElement) {
    const query = inputEl.textContent?.trim() || "";
    this.items = this.filterFn(query, this.availableItems);

    // Hide if no matches
    if (this.items.length === 0) {
      this.hide();
      return;
    }

    this.currentInput = inputEl;
    this.selectedIndex = -1;

    // Build dropdown items
    this.dropdown.innerHTML = "";
    this.items.forEach((item, index) => {
      const itemEl = document.createElement("div");
      itemEl.className = "autocomplete-item";
      itemEl.textContent = item;
      itemEl.onclick = () => this.selectItem(item);
      itemEl.onmouseenter = () => {
        this.selectedIndex = index;
        this.updateHighlight();
      };
      this.dropdown.appendChild(itemEl);
    });

    // Calculate positioning
    const rect = inputEl.getBoundingClientRect();
    const minWidth = Math.max(300, rect.width);

    // Determine if we should show above or below
    const spaceAbove = rect.top;
    const spaceBelow = window.innerHeight - rect.bottom;
    const showBelow = spaceBelow > spaceAbove;

    // Set width
    this.dropdown.style.minWidth = `${minWidth}px`;

    // Position horizontally (ensure it doesn't overflow right edge)
    const maxLeft = window.innerWidth - minWidth - 10;
    const left = Math.min(rect.left, maxLeft);
    this.dropdown.style.left = `${Math.max(0, left)}px`;

    // Position vertically and set max-height based on available space
    if (showBelow) {
      // Show below input
      this.dropdown.style.top = `${rect.bottom + 5}px`;
      this.dropdown.style.bottom = "auto";
      this.dropdown.style.maxHeight = `${spaceBelow - 10}px`;
    } else {
      // Show above input
      this.dropdown.style.bottom = `${window.innerHeight - rect.top + 5}px`;
      this.dropdown.style.top = "auto";
      this.dropdown.style.maxHeight = `${spaceAbove - 10}px`;
    }

    this.dropdown.style.display = "block";
    this.visible = true;
  }

  hide() {
    this.dropdown.style.display = "none";
    this.visible = false;
    this.selectedIndex = -1;
    this.currentInput = null;
  }

  isVisible(): boolean {
    return this.visible;
  }

  updateSelection(direction: "up" | "down") {
    if (!this.visible || this.items.length === 0) return;

    if (direction === "down") {
      this.selectedIndex = (this.selectedIndex + 1) % this.items.length;
    } else {
      this.selectedIndex = this.selectedIndex <= 0 ? this.items.length - 1 : this.selectedIndex - 1;
    }

    this.updateHighlight();
  }

  selectCurrent() {
    const indexToSelect = this.selectedIndex >= 0 ? this.selectedIndex : 0;
    if (this.items[indexToSelect]) {
      this.selectItem(this.items[indexToSelect]);
    } else {
      const inputEl = this.currentInput;
      this.hide();
      inputEl?.blur();
    }
  }

  private selectItem(item: string) {
    if (!this.currentInput) return;

    const inputEl = this.currentInput;

    inputEl.textContent = item;
    // Trigger input event to update state
    inputEl.dispatchEvent(new Event("input"));

    // Clear selection before hiding and blurring
    window.getSelection()?.removeAllRanges();
    this.hide();
    inputEl.blur();

    // Notify parent
    this.onSelect(item);
  }

  private updateHighlight() {
    const items = this.dropdown.querySelectorAll(".autocomplete-item");
    items.forEach((item, index) => {
      if (index === this.selectedIndex) {
        item.classList.add("selected");
        item.scrollIntoView({ block: "nearest" });
      } else {
        item.classList.remove("selected");
      }
    });
  }
}
