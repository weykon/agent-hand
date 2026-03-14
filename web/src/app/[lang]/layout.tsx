import Script from "next/script";
import { RootProvider } from "fumadocs-ui/provider";
import { I18nProvider } from "@/i18n/provider";
import { getDictionary } from "@/i18n";
import { languages } from "@/lib/i18n";
import type { Locale } from "@/lib/i18n";
import { Navbar } from "./navbar";

export function generateStaticParams() {
  return languages.map((lang) => ({ lang }));
}

export default async function LangLayout({
  children,
  params,
}: {
  children: React.ReactNode;
  params: Promise<{ lang: string }>;
}) {
  const { lang } = await params;
  const locale = (languages.includes(lang as Locale) ? lang : "en") as Locale;
  const dict = getDictionary(locale);

  return (
    <>
      <script
        dangerouslySetInnerHTML={{
          __html: `document.documentElement.lang="${locale}";`,
        }}
      />
      <RootProvider theme={{ defaultTheme: "dark", forcedTheme: "dark" }}>
        <I18nProvider dict={dict} lang={locale}>
          <Navbar />
          {children}
        </I18nProvider>
        <Script id="ah-analytics" strategy="afterInteractive">{`
(function(){
  if(location.hostname==='localhost'||location.hostname==='127.0.0.1')return;
  var vid;try{vid=localStorage.getItem('_ah_vid');if(!vid){vid=typeof crypto!=='undefined'&&crypto.randomUUID?crypto.randomUUID():Math.random().toString(36).slice(2)+Date.now().toString(36);localStorage.setItem('_ah_vid',vid);}}catch(e){vid=Math.random().toString(36).slice(2);}
  try{fetch('https://auth.asymptai.com/api/track',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({page:location.pathname,referrer:document.referrer||'',visitor_id:vid}),keepalive:true}).catch(function(){});}catch(e){}
})();
        `}</Script>
      </RootProvider>
    </>
  );
}
