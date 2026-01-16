/**
 * Filters beancount accounts by colon-separated fuzzy matching.
 *
 * Examples:
 *   - "exp:food" matches "Expenses:Food:Groceries"
 *   - "ass:bank" matches "Assets:Bank:Checking"
 *   - Query parts can appear in any order
 */
export function filterAccounts(query: string, accounts: string[]): string[] {
  if (!query) return accounts;

  const queryParts = query
    .toLowerCase()
    .split(":")
    .filter((p) => p.length > 0);

  return accounts
    .filter((account) => {
      const accountParts = account.toLowerCase().split(":");

      // Every query part must match at least one account part (as prefix)
      return queryParts.every((queryPart) =>
        accountParts.some((accountPart) => accountPart.startsWith(queryPart)),
      );
    })
    .sort((a, b) => {
      // Prioritize matches where query parts appear in order
      const aLower = a.toLowerCase();
      const bLower = b.toLowerCase();

      const aInOrder = matchesInOrder(queryParts, aLower.split(":"));
      const bInOrder = matchesInOrder(queryParts, bLower.split(":"));

      if (aInOrder && !bInOrder) return -1;
      if (!aInOrder && bInOrder) return 1;

      return a.localeCompare(b);
    });
}

function matchesInOrder(queryParts: string[], accountParts: string[]): boolean {
  let accountIndex = 0;
  for (const queryPart of queryParts) {
    while (accountIndex < accountParts.length) {
      if (accountParts[accountIndex].startsWith(queryPart)) {
        accountIndex++;
        break;
      }
      accountIndex++;
    }
    if (
      accountIndex === accountParts.length &&
      !accountParts[accountParts.length - 1]?.startsWith(queryPart)
    ) {
      return false;
    }
  }
  return true;
}
