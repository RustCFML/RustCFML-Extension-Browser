<cfscript>
targets = [ "https://blog.rust-lang.org/", "file://#getDirectoryFromPath( getCurrentTemplatePath() )#/brand.html", "https://news.ycombinator.com/" ];
for ( u in targets ) {
  p = Browser({timeout:60000}).newPage().goto( u ).settle( 1200 );
  f = p.fonts();
  writeOutput( u & chr(10) );
  writeOutput( "   LOADED:      " );
  if ( arrayLen(f.loaded) ) { for ( s in f.loaded ) writeOutput( s.family & "(" & s.elements & "el, " & s.width & "px) " ); } else writeOutput( "-" );
  writeOutput( chr(10) & "   SUBSTITUTED: " );
  if ( arrayLen(f.substituted) ) { for ( s in f.substituted ) writeOutput( s.family & " -> " & s.renderedAs & " (" & s.elements & "el) " ); } else writeOutput( "none" );
  writeOutput( chr(10) & "   @font-face:  " & ( arrayLen(f.webFonts) ? arrayToList(f.webFonts,", ") : "none" ) & chr(10) & chr(10) );
  p.close();
}
</cfscript>
