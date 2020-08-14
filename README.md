# Backup GUI
A functioning backup tool with a GUI, written in Rust for the Backblaze's B2 Cloud Storage.

## Requirements
Requires a Vulkan driver on your system.   
This should already be present on Windows, but you may need to install the package for your Linux distro.

You'll also need an account on Backblaze. See https://www.backblaze.com/b2/cloud-storage.html for signup and pricing.

## Setup
You'll need to do a bit of setup in Backblaze before you can use this program.  
First you should create a bucket, then you should generate a new app key. 
It it suggested that you change the lifecycle settings of the bucket to " Keep prior versions for this number of days".

You'll need to generate a new application key. Go into 'App Keys' and press 'Add a New Application Key'
![](1.png)
  
You can give the key a name and optionally make it only work for your new bucket and more. 
![](2.png)

Now you're presented with the app key secret:
![](3.png)
Note that the 'applicationKey' will only be shown once. If you lose it, you need to make a new key.

Lastly you need to grab the ID of the bucket you created, which can be read in the menu:
![](4.png)

## Disclaimer
This project is not associated with Backblaze. Use at your own risk. See License.md.    
