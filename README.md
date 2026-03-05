[<img src="https://raw.githubusercontent.com/PonderForge/LBProxy/main/images/Logo.png" width="400">](https://raw.githubusercontent.com/PonderForge/LustBlock/main/images/Logo.png)\
Ridculously High Speed NSFW MITM Internet Filter 


https://github.com/user-attachments/assets/9898d0b6-6543-479d-b940-270d4c123bc7


# Install
## Linux
1. Download from releases: lbproxy-linux.deb or lbproxy-linux.tar.xz
2. Install/Extract
3. Run LBProxy, either by typing lb-dashbaord for the tar or lbproxy for the deb
3. Once installed open up the dashboard and mess with settings towards your liking. I suggest enabling the autoconnect and start on boot to filter traffic automatically, via the Post Install.
4. Run the Proxy
## Windows
1. Download from releases: lb-proxy-windows.exe; or Download lbproxy-windows.zip and skip to step 3
2. Follow instructions through the prompts
3. Once installed open up the dashboard and mess with settings towards your liking. I suggest enabling the autoconnect and start on boot to filter traffic automatically, via the Post Install.
4. Run the Proxy
## Post Install
1. Configure your operating system to use the proxy, via the server's configured IP address in Advanced:
- On Ubuntu, follow [this tutorial](https://phoenixnap.com/kb/ubuntu-proxy-settings)
- On Windows, follow [this tutorial](https://support.microsoft.com/en-us/windows/use-a-proxy-server-in-windows-03096c53-0554-4ffe-b6ab-8b1deee8dae1)
- On Android, follow [this tutorial](https://proxyway.com/guides/android-proxy-settings)
- On iOS, follow [this tutorial](https://libertyshield.kayako.com/article/32-manual-proxy-ios-iphone-and-ipad)
2. Finally, to scan HTTPS and to prevent your browser from going into lockdown, install pub.crt, a fake cert, which is in your OS's config directory, which is either `C:/Users/{user}/AppData/Roaming/LBProxy` or `/home/{user}/.config/LBProxy`:
- On Ubuntu, follow [this tutorial](https://askubuntu.com/questions/73287/how-do-i-install-a-root-certificate/94861#94861)
- On Windows, follow [this tutorial](https://web.archive.org/web/20160612045445/http://windows.microsoft.com/en-ca/windows/import-export-certificates-private-keys#1TC=windows-7)
- On Android, follow [this tutorial](http://wiki.cacert.org/FAQ/ImportRootCert#Android_Phones_.26_Tablets)
- On iOS, follow [this tutorial](http://jasdev.me/intercepting-ios-traffic)
# Compiling
If you want to compile with more of ONNX's Execution providers such as DirectX or CUDA, or you just want to mess with the code, follow these steps.
1. Download the source code either via GitHub desktop, or by running:\
`git clone https://github.com/PonderForge/LBProxy.git && cd LBProxy`
2. Write any edits to the code as you wish
3. Next use Cargo to build via:\
`cargo build --release`
4. Go to the target/release directory and run LBProxy

# TODO
1. Add Human Segmentation to give people clothes where they need it (Cause they're too lazy to do it themselves?)
2. Video Scanning: It'll be slow as molasses but hey, sexual free content!
3. Text Scanning: Using FastText and OCR
4. Encrypted Porn Website blocker: Since many websites are 100% porn, we want to add a encrypted list of websites that should be completely blocked and blacklisted.
8. Scan POST requests: Cause people also send out images, we need to scan those for sexting, and other forms of porn and sexual images.
# Contributing
I know that there's a lot of bugs, but that's were you come in! I need beta testers, programmers, hackers, etcetera, to find problems. Beta testers, please don't purposely find sexy images, but if you happen to come across them with LustBlock on, submit a issue (just not the problematic photo)! Programmers, I'm just one man, plus my calculus professor keeps giving me homework, so please, if you have optimizations, submit a issue! Hackers, I know the program is insecure, so make it secure! I'm not a expert, but I am willing to work with people! THX!
If you want to donate, donate to my church at [firste.org](https://firste.org).
# Why?
Since you are this far into the README, you must be interested in the motivation for this project.
Lust by definition is "very strong sexual desire" for another person as almost everyone experiences. Left unchecked, it starts to destroy you internally which will begin to affect others around you. Porn and Sexual images are one of the easiest things to lust after because it is cheap, hideable, and spreads faster than my dog can run. Even a simple Google search could bring up results that could spiral you down a path of torment and destruction, just due to the girl or boy that was taught that they are nothing more than meat. That is why LustBlock was created. Thanks to the help of GantMan's NSFW models, we can quickly and accurately detect and remove NSFW images from the web before it reaches you and your loved ones. Did you know that when one starts to see porn for "fullfillment", their chance of getting a divorce rate [doubles](https://www.science.org/content/article/divorce-rates-double-when-people-start-watching-porn)? If you have not had to fight lust, you are very lucky. It is literally the [cocaine of the internet](https://www.provenmen.org/porn-damages-brain/). Imagine if your brother, or sister, or your wife, or your husband, or your favorite teacher was forced to pose, naked, for a bunch of people cause it's "just a little fun". Imagine if you had to do that. Humans were not meant for that. But because we corrupted ourselves we have spread torment and pain everywhere just for a bit of pleasure. Something warned us about that. Oh, yeah. It was the [Bible](https://www.bible.com/) and literally thousands of years of history.
# Credits
- [hatoo/http-mitm-proxy](https://github.com/hatoo/http-mitm-proxy/tree/master): I modified his library for the HTTPS proxy! Couldn't have done it without this project.
- [PonderForge/openlb](https://github.com/PonderForge/Openlb): This is the actual filter, acting as my side project, and contains most of the credits for the actual scanning. Check it out if you want to use it in a personal project, or just want to know how the scanning works.
- Jesus Christ: My savior, my redeemer, my rock, my king, my commander, and literally the sole reason I exist. Wrote the book that warned us about lust and still loves us when we ignore it. All Hail King Jesus!\
\
This was created by PonderForge, if you use this code, give credit where credit is due.\
Pslam 111:2 "Great are the works of the LORD; they are pondered by all who delight in them."
