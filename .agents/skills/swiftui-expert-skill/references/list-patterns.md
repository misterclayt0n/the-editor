# SwiftUI List Patterns Reference

## ForEach Identity and Stability

**Always provide stable identity for `ForEach`.** Never use `.indices` for dynamic content.

```swift
// Good - stable identity via Identifiable
extension User: Identifiable {
    var id: String { userId }
}

ForEach(users) { user in
    UserRow(user: user)
}

// Good - stable identity via keypath
ForEach(users, id: \.userId) { user in
    UserRow(user: user)
}

// Wrong - indices create static content
ForEach(users.indices, id: \.self) { index in
    UserRow(user: users[index])  // Can crash on removal!
}

// Wrong - unstable identity
ForEach(users, id: \.self) { user in
    UserRow(user: user)  // Only works if User is Hashable and stable
}
```

**Critical**: Ensure **constant number of views per element** in `ForEach`:

```swift
// Good - consistent view count
ForEach(items) { item in
    ItemRow(item: item)
}

// Bad - variable view count breaks identity
ForEach(items) { item in
    if item.isSpecial {
        SpecialRow(item: item)
        DetailRow(item: item)
    } else {
        RegularRow(item: item)
    }
}
```

**Avoid inline filtering:**

```swift
// Bad - unstable identity, changes on every update
ForEach(items.filter { $0.isEnabled }) { item in
    ItemRow(item: item)
}

// Good - prefilter and cache
@State private var enabledItems: [Item] = []

var body: some View {
    ForEach(enabledItems) { item in
        ItemRow(item: item)
    }
    .onChange(of: items) { _, newItems in
        enabledItems = newItems.filter { $0.isEnabled }
    }
}
```

**Avoid `AnyView` in list rows:**

```swift
// Bad - hides identity, increases cost
ForEach(items) { item in
    AnyView(item.isSpecial ? SpecialRow(item: item) : RegularRow(item: item))
}

// Good - Create a unified row view
ForEach(items) { item in
    ItemRow(item: item)
}

struct ItemRow: View {
    let item: Item

    var body: some View {
        if item.isSpecial {
            SpecialRow(item: item)
        } else {
            RegularRow(item: item)
        }
    }
}
```

**Why**: Stable identity is critical for performance and animations. Unstable identity causes excessive diffing, broken animations, and potential crashes.

### Identifiable ID Must Be Truly Unique

Non-unique IDs cause SwiftUI to treat different items as identical, leading to duplicate rendering or missing views:

```swift
// Bug -- two articles with the same URL show identical content
struct Article: Identifiable {
    let title: String
    let url: URL
    var id: String { url.absoluteString }  // Not unique if URLs repeat!
}

// Fix -- use a genuinely unique identifier
struct Article: Identifiable {
    let id: UUID
    let title: String
    let url: URL
}
```

**Classes get a default `ObjectIdentifier`-based `id`** when conforming to `Identifiable` without providing one. This is only unique for the object's lifetime and can be recycled after deallocation.

## Enumerated Sequences

**Always convert enumerated sequences to arrays. To be able to use them in a ForEach.**

```swift
let items = ["A", "B", "C"]

// Correct
ForEach(Array(items.enumerated()), id: \.offset) { index, item in
    Text("\(index): \(item)")
}

// Wrong - Doesn't compile, enumerated() isn't an array
ForEach(items.enumerated(), id: \.offset) { index, item in
    Text("\(index): \(item)")
}
```

## List with Custom Styling

```swift
// Remove default background and separators
List(items) { item in
    ItemRow(item: item)
        .listRowInsets(EdgeInsets(top: 8, leading: 16, bottom: 8, trailing: 16))
        .listRowSeparator(.hidden)
}
.listStyle(.plain)
.scrollContentBackground(.hidden)
.background(Color.customBackground)
.environment(\.defaultMinListRowHeight, 1)  // Allows custom row heights
```

## List with Pull-to-Refresh

```swift
List(items) { item in
    ItemRow(item: item)
}
.refreshable {
    await loadItems()
}
```

## Empty States with ContentUnavailableView (iOS 17+)

Use `ContentUnavailableView` for empty list/search states. The built-in `.search` variant is auto-localized:

```swift
List {
    ForEach(searchResults) { item in
        ItemRow(item: item)
    }
}
.overlay {
    if searchResults.isEmpty, !searchText.isEmpty {
        ContentUnavailableView.search(text: searchText)
    }
}
```

For non-search empty states, use a custom instance:

```swift
ContentUnavailableView(
    "No Articles",
    systemImage: "doc.richtext.fill",
    description: Text("Articles you save will appear here.")
)
```

## Custom List Backgrounds

Use `.scrollContentBackground(.hidden)` to replace the default list background:

```swift
List(items) { item in
    ItemRow(item: item)
}
.scrollContentBackground(.hidden)
.background(Color.customBackground)
```

Without `.scrollContentBackground(.hidden)`, a custom `.background()` has no visible effect on `List`.

## Summary Checklist

- [ ] ForEach uses stable identity (never `.indices` for dynamic content)
- [ ] Identifiable IDs are truly unique across all items
- [ ] Constant number of views per ForEach element
- [ ] No inline filtering in ForEach (prefilter and cache instead)
- [ ] No `AnyView` in list rows
- [ ] Don't convert enumerated sequences to arrays
- [ ] Use `.refreshable` for pull-to-refresh
- [ ] Use `ContentUnavailableView` for empty states (iOS 17+)
- [ ] Use `.scrollContentBackground(.hidden)` for custom list backgrounds
