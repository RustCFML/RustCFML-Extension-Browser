<cfscript>
// Several live pages at once. Before obscura #824 (issue #756), dropping a page
// while another was alive aborted the whole process with a V8 fatal, so this
// file either prints its summary or does not finish at all.
here = "file://#getDirectoryFromPath( getCurrentTemplatePath() )#";
fails = 0;
function check( label, got, want ) {
  if ( got == want ) { writeOutput( "  ok    " & label & chr(10) ); }
  else { writeOutput( "  FAIL  " & label & " -> got [" & got & "], want [" & want & "]" & chr(10) ); fails++; }
}

b = Browser();
for ( round = 1; round <= 3; round++ ) {
  first  = b.newPage().goto( here & "two.html" );
  second = b.newPage().goto( here & "hist.html" );
  third  = b.newPage().goto( here & "app.html" );
  first.close();
  check( "round #round#: pages outlive a closed sibling", second.title() & "/" & third.title(), "one/app" );
  third.close();
  check( "round #round#: and the last one still works", second.title(), "one" );
  second.close();
}

other = Browser();
mine   = b.newPage().goto( here & "two.html" );
theirs = other.newPage().goto( here & "two.html" );
mine.close();
check( "a page in a second browser outlives one in the first", theirs.title(), "two" );
theirs.close();

writeOutput( fails ? "SOME FAILED" & chr(10) : "ALL PAGES OK" & chr(10) );
</cfscript>
