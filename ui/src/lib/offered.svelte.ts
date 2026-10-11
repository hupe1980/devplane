// What the page on screen offers the palette: a document's own actions (a
// change's Review, Run gates, Offer), each run through that document's own
// code and so its own guard. The palette lists what is here and nothing
// else; a page that leaves takes its commands with it.

export type Command = { label: string; icon: string; hint?: string; run: () => void };

export const here = $state<{ title: string; noun: string; list: Command[]; by: number }>({ title: "", noun: "", list: [], by: 0 });

let offers = 0;

/// Offers these commands while the caller is on screen; returns the
/// withdrawal, for an `$effect`. `noun` heads them in the palette (*This
/// change*, *This item*).
export function offerCommands(title: string, list: Command[], noun = "This change"): () => void {
  const by = ++offers;
  here.title = title;
  here.noun = noun;
  here.list = list;
  here.by = by;
  return () => {
    if (here.by === by) {
      here.title = "";
      here.noun = "";
      here.list = [];
      here.by = 0;
    }
  };
}
