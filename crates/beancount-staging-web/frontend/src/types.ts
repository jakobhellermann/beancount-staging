export interface Directive {
  id: string;
  transaction: Transaction;
}

export interface Transaction {
  date: string;
  flag: string;
  payee: string | null;
  narration: string | null;
  tags: string[];
  links: string[];
  postings: Posting[];
}

export interface Posting {
  account: string;
  amount: Amount | null;
  cost: string | null;
  price: string | null;
}

export interface Amount {
  value: string;
  currency: string;
}
